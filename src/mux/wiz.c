/*
 * wiz.c -- Wizard-only commands
 */

#include "config.h"

#include "alloc.h"
#include "attrs.h"
#include "command.h"
#include "config.h"
#include "db.h"
#include "externs.h"
#include "file_c.h"
#include "match.h"
#include "mudconf.h"
#include "powers.h"
#include "rbtab.h"

extern char *crypt(const char *, const char *);

void do_teleport(dbref player, dbref cause, int key, char *arg1, char *arg2) {
  dbref victim, destination, loc, exitloc;
  char *to;
  int hush = 0;

  if (((Fixed(player)) || (Fixed(Owner(player)))) && !Wizard(player)) {
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

  if (!Has_location(victim) && Typeof(victim) != TYPE_EXIT) {
    notify_quiet(player, "You can't teleport that.");
    return;
  }
  /*
   * Fail if we don't control the victim or the victim's location
   */

  if (!Controls(player, victim) && !Controls(player, Location(victim)) &&
      !Wizard(player)) {
    notify_quiet(player, "Permission denied.");
    return;
  }
  /*
   * Check for teleporting home
   * Also, can't teleport exits 'home'
   */

  if (!string_compare(to, "home") && Typeof(victim) != TYPE_EXIT) {
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
    if (!Good_obj(loc) || !isRoom(loc) ||
        (!Controls(player, loc) && !Wizard(player))) {
      notify_quiet(player, "Permission denied.");
      return;
    }
  }
  if (Has_contents(destination)) {

    /*
     * You must control the destination and pass its TELEPORT lock.
     */

    if ((!Controls(player, destination) && !Wizard(player)) ||
        !could_doit(player, destination, A_LTPORT)) {

      /*
       * Nope, report failure
       */

      if (player != victim)
        notify_quiet(player, "Permission denied.");
      did_it(victim, destination, A_TFAIL, "You can't teleport there!",
             A_OTFAIL, 0, A_ATFAIL, (char **)NULL, 0);
      return;
    }
    /*
     * We're OK, do the teleport
     */

    if ((key & TELEPORT_QUIET) || Dark(victim))
      hush = HUSH_ENTER | HUSH_LEAVE;

    if (Typeof(victim) == TYPE_EXIT) {
      exitloc = Exits(victim);
      s_Exits(exitloc, remove_first(Exits(exitloc), victim));
      s_Exits(destination, insert_first(Exits(destination), victim));
      s_Exits(victim, destination);

      if (!Quiet(player))
        notify_quiet(player, "Exit teleported.");
    } else if (move_via_teleport(victim, destination, cause, hush)) {
      if (player != victim && !Quiet(player))
        notify_quiet(player, "Teleported.");
    }
  } else if (isExit(destination)) {
    if (Exits(destination) == Location(victim)) {
      move_exit(victim, destination, 0, "You can't go that way.", 0);
    } else {
      notify_quiet(player, "I can't find that exit.");
    }
  }
}

/**
 * Interlude to do_force for the # command
 */
void do_force_prefixed(dbref player, dbref cause, int key, char *command,
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
void do_force(dbref player, dbref cause, int key, char *what, char *command,
              char *args[], int nargs) {
  dbref victim;

  if ((victim = match_controlled(player, what)) == NOTHING)
    return;

  /*
   * force victim to do command
   */

  wait_que(victim, player, 0, NOTHING, 0, command, args, nargs,
           mudstate.global_regs);
}

void do_newpassword(dbref player, dbref cause, int key, char *name,
                    char *password) {
  dbref victim;
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
  if (God(victim)) {
    notify_quiet(player, "You cannot change that player's password.");
    return;
  }
  STARTLOG(LOG_WIZARD, "WIZ", "PASS") {
    log_name(player);
    log_text((char *)" changed the password of ");
    log_name(victim);
    ENDLOG;
  }
  /*
   * it's ok, do it
   */
  s_Pass(victim, crypt((const char *)password, "XX"));
  buf = alloc_lbuf("do_newpassword");
  notify_quiet(player, "Password changed.");
  snprintf(buf, LBUF_SIZE, "Your password has been changed by %s.",
           Name(player));
  notify_quiet(victim, buf);
  free_lbuf(buf);
}

void do_boot(dbref player, dbref cause, int key, char *name) {
  dbref victim;
  char *buf, *bp;
  int count;

  if (!Wizard(player)) {
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
      log_text((char *)" was @booted by ");
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

    if (God(victim)) {
      notify_quiet(player, "You cannot boot that player!");
      return;
    }
    if ((!isPlayer(victim) && !God(player)) || (player == victim)) {
      notify_quiet(player, "You can only boot off other players!");
      return;
    }
    STARTLOG(LOG_WIZARD, "WIZ", "BOOT") {
      log_name_and_loc(victim);
      log_text((char *)" was @booted by ");
      log_name(player);
      ENDLOG;
    }
    notify_quiet(player, tprintf("You booted %s off!", Name(victim)));
  }
  if (key & BOOT_QUIET) {
    buf = NULL;
  } else {
    bp = buf = alloc_lbuf("do_boot.msg");
    safe_str(Name(player), buf, &bp);
    safe_str((char *)" gently shows you the door.", buf, &bp);
    *bp = '\0';
  }

  if (key & BOOT_PORT)
    count = boot_by_port(victim, !God(player), buf);
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
void do_cut(dbref player, dbref cause, int key, char *thing) {
  dbref object;

  object = match_controlled(player, thing);
  switch (object) {
  case NOTHING:
    notify_quiet(player, "No match.");
    break;
  case AMBIGUOUS:
    notify_quiet(player, "I don't know which one");
    break;
  default:
    s_Next(object, NOTHING);
    notify_quiet(player, "Cut.");
  }
}

/**
 * Enable or disable global control flags
 */
NAMETAB enable_names[] = {
    {(char *)"building", 1, CA_PUBLIC, CF_BUILD},
    {(char *)"checkpointing", 2, CA_PUBLIC, CF_CHECKPOINT},
    {(char *)"cleaning", 2, CA_PUBLIC, CF_DBCHECK},
    {(char *)"dequeueing", 1, CA_PUBLIC, CF_DEQUEUE},
    {(char *)"idlechecking", 2, CA_PUBLIC, CF_IDLECHECK},
    {(char *)"interpret", 2, CA_PUBLIC, CF_INTERP},
    {(char *)"logins", 3, CA_PUBLIC, CF_LOGIN},
    {(char *)"eventchecking", 2, CA_PUBLIC, CF_EVENTCHECK},
    {NULL, 0, 0, 0}};

void do_global(dbref player, dbref cause, int key, char *flag) {
  int flagvalue;

  /*
   * Set or clear the indicated flag
   */

  flagvalue = search_nametab(player, enable_names, flag);
  if (flagvalue < 0) {
    notify_quiet(player, "I don't know about that flag.");
  } else if (key == GLOB_ENABLE) {
    mudconf.control_flags |= flagvalue;
    if (!Quiet(player))
      notify_quiet(player, "Enabled.");
  } else if (key == GLOB_DISABLE) {
    mudconf.control_flags &= ~flagvalue;
    if (!Quiet(player))
      notify_quiet(player, "Disabled.");
  } else {
    notify_quiet(player, "Illegal combination of switches.");
  }
}
