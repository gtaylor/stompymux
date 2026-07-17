/*
 * wiz.c -- Wizard-only commands
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/file_cache.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/password.h"
#include "mux/world/match.h"

void do_teleport(DbRef player, DbRef cause, int key, char *arg1, char *arg2) {
  DbRef victim, destination, loc, exitloc;
  char *to;
  int hush = 0;

  if (((is_fixed(player)) || (is_fixed(obj_owner(player)))) &&
      !is_wizard(player)) {
    notify(player, mudconf.fixed_tel_msg);
    return;
  }
  /*
   * get victim
   */

  if (*arg2 == '\0') {
    victim = player;
    to = arg1;
  } else {
    init_match(player, arg1, NOTYPE);
    match_everything(0);
    victim = noisy_match_result();

    if (victim == NOTHING)
      return;
    to = arg2;
  }

  /*
   * Validate type of victim
   */

  if (!has_location(victim) && typeof_obj(victim) != TYPE_EXIT) {
    notify_quiet(player, "You can't teleport that.");
    return;
  }
  /*
   * Fail if we don't control the victim or the victim's location
   */

  if (!is_controls(player, victim) &&
      !is_controls(player, obj_location(victim)) && !is_wizard(player)) {
    notify_quiet(player, "Permission denied.");
    return;
  }
  /*
   * Check for teleporting home
   * Also, can't teleport exits 'home'
   */

  if (!string_compare(to, "home") && typeof_obj(victim) != TYPE_EXIT) {
    (void)move_via_teleport(victim, HOME, cause, 0);
    return;
  }
  /*
   * Find out where to send the victim
   */

  init_match(player, to, NOTYPE);
  match_everything(0);
  destination = match_result();

  switch (destination) {
  case NOTHING:
    notify_quiet(player, "No match.");
    return;
  case AMBIGUOUS:
    notify_quiet(player, "I don't know which destination you mean!");
    return;
  default:
    if (victim == destination) {
      notify_quiet(player, "Bad destination.");
      return;
    }
  }

  /*
   * If fascist teleport is on, you must control the victim's ultimate
   * location (after LEAVEing any objects).
   */

  if (mudconf.fascist_tport) {
    loc = where_room(victim);
    if (!is_good_obj(loc) || !is_room(loc) ||
        (!is_controls(player, loc) && !is_wizard(player))) {
      notify_quiet(player, "Permission denied.");
      return;
    }
  }
  if (has_contents(destination)) {

    /*
     * You must control the destination and pass its TELEPORT lock.
     */

    if ((!is_controls(player, destination) && !is_wizard(player)) ||
        !could_doit(player, destination, A_LTPORT)) {

      /*
       * Nope, report failure
       */

      if (player != victim)
        notify_quiet(player, "Permission denied.");
      did_it(victim, destination, A_TFAIL, "You can't teleport there!",
             A_OTFAIL, 0, A_ATFAIL, (char **)nullptr, 0);
      return;
    }
    /*
     * We're OK, do the teleport
     */

    if ((key & TELEPORT_QUIET) || is_dark(victim))
      hush = HUSH_ENTER | HUSH_LEAVE;

    if (typeof_obj(victim) == TYPE_EXIT) {
      exitloc = obj_exits(victim);
      s_exits(exitloc, remove_first(obj_exits(exitloc), victim));
      s_exits(destination, insert_first(obj_exits(destination), victim));
      s_exits(victim, destination);

      if (!is_quiet(player))
        notify_quiet(player, "Exit teleported.");
    } else if (move_via_teleport(victim, destination, cause, hush)) {
      if (player != victim && !is_quiet(player))
        notify_quiet(player, "Teleported.");
    }
  } else if (is_exit(destination)) {
    if (obj_exits(destination) == obj_location(victim)) {
      move_exit(victim, destination, 0, "You can't go that way.", 0);
    } else {
      notify_quiet(player, "I can't find that exit.");
    }
  }
}

/**
 * Interlude to do_force for the # command
 */
void do_force_prefixed(DbRef player, DbRef cause, int key, char *command,
                       char *args[], int nargs) {
  char *cp;

  cp = parse_to(&command, ' ', 0);
  if (!command)
    return;
  while (*command && isspace(*command))
    command++;
  if (*command)
    do_force(player, cause, key, cp, command, args, nargs);
}

/**
 * Force an object to do something.
 */
void do_force(DbRef player, DbRef cause, int key, char *what, char *command,
              char *args[], int nargs) {
  DbRef victim;

  if ((victim = match_controlled(player, what)) == NOTHING)
    return;

  /*
   * force victim to do command
   */

  wait_que(victim, player, 0, NOTHING, 0, command, args, nargs,
           mudstate.global_regs);
}

void do_newpassword(DbRef player, DbRef cause, int key, char *name,
                    char *password) {
  DbRef victim;
  char hashed_password[crypto_pwhash_STRBYTES];
  char *buf;

  if ((victim = lookup_player(player, name, 0)) == NOTHING) {
    notify_quiet(player, "No such player.");
    return;
  }
  if (*password != '\0' && !ok_password(password)) {

    /*
     * Can set null passwords, but not bad passwords
     */
    notify_quiet(player, "Bad password");
    return;
  }
  if (is_god(victim)) {
    notify_quiet(player, "You cannot change that player's password.");
    return;
  }
  if (!password_hash(password, hashed_password)) {
    notify_quiet(player, "Unable to change password.");
    return;
  }
  object_password_set(victim, hashed_password);
  sodium_memzero(hashed_password, sizeof(hashed_password));
  STARTLOG(LOG_WIZARD, "WIZ", "PASS") {
    log_name(player);
    log_text(" changed the password of ");
    log_name(victim);
    ENDLOG;
  }
  buf = alloc_lbuf("do_newpassword");
  notify_quiet(player, "Password changed.");
  snprintf(buf, LBUF_SIZE, "Your password has been changed by %s.",
           Name(player));
  notify_quiet(victim, buf);
  free_lbuf(buf);
}

void do_boot(DbRef player, DbRef cause, int key, char *name) {
  DbRef victim;
  char *buf, *bp;
  int count;

  if (!is_wizard(player)) {
    notify(player, "Permission denied.");
    return;
  }
  if (key & BOOT_PORT) {
    if (is_number(name)) {
      victim = atoi(name);
    } else {
      notify_quiet(player, "That's not a number!");
      return;
    }
    STARTLOG(LOG_WIZARD, "WIZ", "BOOT") {
      buf = alloc_sbuf("do_boot.port");
      snprintf(buf, SBUF_SIZE, "Port %ld", victim);
      log_text(buf);
      log_text(" was @booted by ");
      log_name(player);
      free_sbuf(buf);
      ENDLOG;
    }
  } else {
    init_match(player, name, TYPE_PLAYER);
    match_neighbor();
    match_absolute();
    match_player();
    if ((victim = noisy_match_result()) == NOTHING)
      return;

    if (is_god(victim)) {
      notify_quiet(player, "You cannot boot that player!");
      return;
    }
    if ((!is_player(victim) && !is_god(player)) || (player == victim)) {
      notify_quiet(player, "You can only boot off other players!");
      return;
    }
    STARTLOG(LOG_WIZARD, "WIZ", "BOOT") {
      log_name_and_loc(victim);
      log_text(" was @booted by ");
      log_name(player);
      ENDLOG;
    }
    notify_quiet(player, tprintf("You booted %s off!", Name(victim)));
  }
  if (key & BOOT_QUIET) {
    buf = nullptr;
  } else {
    bp = buf = alloc_lbuf("do_boot.msg");
    safe_str(Name(player), buf, &bp);
    safe_str(" gently shows you the door.", buf, &bp);
    *bp = '\0';
  }

  if (key & BOOT_PORT)
    count = boot_by_port((int)victim, !is_god(player), buf);
  else
    count = boot_off(victim, buf);
  notify_quiet(player, tprintf("%d connection%s closed.", count,
                               (count == 1 ? "" : "s")));
  if (buf)
    free_lbuf(buf);
}

/**
 * Reduce the wealth of anyone over a specified amount.
 */
/**
 * Chop off a contents or exits chain after the named item.
 */
void do_cut(DbRef player, DbRef cause, int key, char *thing) {
  DbRef object;

  object = match_controlled(player, thing);
  switch (object) {
  case NOTHING:
    notify_quiet(player, "No match.");
    break;
  case AMBIGUOUS:
    notify_quiet(player, "I don't know which one");
    break;
  default:
    s_next(object, NOTHING);
    notify_quiet(player, "Cut.");
  }
}

/**
 * Enable or disable global control flags
 */
NameTable enable_names[] = {{"checkpointing", 2, CA_PUBLIC, CF_CHECKPOINT},
                            {"cleaning", 2, CA_PUBLIC, CF_DBCHECK},
                            {"dequeueing", 1, CA_PUBLIC, CF_DEQUEUE},
                            {"idlechecking", 2, CA_PUBLIC, CF_IDLECHECK},
                            {"interpret", 2, CA_PUBLIC, CF_INTERP},
                            {"logins", 3, CA_PUBLIC, CF_LOGIN},
                            {"eventchecking", 2, CA_PUBLIC, CF_EVENTCHECK},
                            {nullptr, 0, 0, 0}};

void do_global(DbRef player, DbRef cause, int key, char *flag) {
  int flagvalue;

  /*
   * Set or clear the indicated flag
   */

  flagvalue = name_table_search(player, enable_names, flag);
  if (flagvalue < 0) {
    notify_quiet(player, "I don't know about that flag.");
  } else if (key == GLOB_ENABLE) {
    mudconf.control_flags |= flagvalue;
    if (!is_quiet(player))
      notify_quiet(player, "Enabled.");
  } else if (key == GLOB_DISABLE) {
    mudconf.control_flags &= ~flagvalue;
    if (!is_quiet(player))
      notify_quiet(player, "Disabled.");
  } else {
    notify_quiet(player, "Illegal combination of switches.");
  }
}
