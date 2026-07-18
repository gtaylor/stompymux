/*
 * wiz.c -- Wizard-only commands
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/file_cache.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/password.h"
#include "mux/world/match.h"

void do_teleport(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  int key = invocation->key;
  char *arg1 = invocation->first;
  char *arg2 = invocation->second;
  ServerConfiguration *configuration =
      invocation->context->server->configuration;
  DbRef victim, destination, loc, exitloc;
  char *to;
  int hush = 0;

  if (((is_fixed(evaluation->world->database, player)) ||
       (is_fixed(evaluation->world->database,
                 game_object_owner(evaluation->world->database, player)))) &&
      !is_wizard(evaluation->world->database, player)) {
    notify(evaluation, player, configuration->fixed_tel_msg);
    return;
  }
  /*
   * get victim
   */

  if (*arg2 == '\0') {
    victim = player;
    to = arg1;
  } else {
    init_match(&invocation->context->match, player, arg1, NOTYPE);
    match_everything(&invocation->context->match, 0);
    victim = noisy_match_result(&invocation->context->match);

    if (victim == NOTHING)
      return;
    to = arg2;
  }

  /*
   * Validate type of victim
   */

  if (!has_location(evaluation->world->database, victim) &&
      typeof_obj(evaluation->world->database, victim) != TYPE_EXIT) {
    notify_quiet(evaluation, player, "You can't teleport that.");
    return;
  }
  /*
   * Fail if we don't control the victim or the victim's location
   */

  if (!is_controls(evaluation, player, victim) &&
      !is_controls(evaluation, player,
                   game_object_location(evaluation->world->database, victim)) &&
      !is_wizard(evaluation->world->database, player)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  /*
   * Check for teleporting home
   * Also, can't teleport exits 'home'
   */

  if (!string_compare(configuration, to, "home") &&
      typeof_obj(evaluation->world->database, victim) != TYPE_EXIT) {
    (void)move_via_teleport(evaluation, victim, HOME, cause, 0);
    return;
  }
  /*
   * Find out where to send the victim
   */

  init_match(&invocation->context->match, player, to, NOTYPE);
  match_everything(&invocation->context->match, 0);
  destination = match_result(&invocation->context->match);

  switch (destination) {
  case NOTHING:
    notify_quiet(evaluation, player, "No match.");
    return;
  case AMBIGUOUS:
    notify_quiet(evaluation, player,
                 "I don't know which destination you mean!");
    return;
  default:
    if (victim == destination) {
      notify_quiet(evaluation, player, "Bad destination.");
      return;
    }
  }

  /*
   * If fascist teleport is on, you must control the victim's ultimate
   * location (after LEAVEing any objects).
   */

  if (configuration->fascist_tport) {
    loc = where_room(evaluation->world->database, configuration, victim);
    if (!is_good_obj(evaluation->world->database, loc) ||
        !is_room(evaluation->world->database, loc) ||
        (!is_controls(evaluation, player, loc) &&
         !is_wizard(evaluation->world->database, player))) {
      notify_quiet(evaluation, player, "Permission denied.");
      return;
    }
  }
  if (has_contents(evaluation->world->database, destination)) {

    /*
     * You must control the destination and pass its TELEPORT lock.
     */

    if ((!is_controls(evaluation, player, destination) &&
         !is_wizard(evaluation->world->database, player)) ||
        !could_doit_with_context(evaluation, player, destination, A_LTPORT)) {

      /*
       * Nope, report failure
       */

      if (player != victim)
        notify_quiet(evaluation, player, "Permission denied.");
      did_it(evaluation, victim, destination, A_TFAIL,
             "You can't teleport there!", A_OTFAIL, 0, A_ATFAIL,
             (char **)nullptr, 0);
      return;
    }
    /*
     * We're OK, do the teleport
     */

    if ((key & TELEPORT_QUIET) || is_dark(evaluation->world->database, victim))
      hush = HUSH_ENTER | HUSH_LEAVE;

    if (typeof_obj(evaluation->world->database, victim) == TYPE_EXIT) {
      exitloc = game_object_exits(evaluation->world->database, victim);
      game_object_set_exits(
          evaluation->world->database, exitloc,
          remove_first(evaluation->world->database,
                       game_object_exits(evaluation->world->database, exitloc),
                       victim));
      game_object_set_exits(
          evaluation->world->database, destination,
          insert_first(
              evaluation->world->database,
              game_object_exits(evaluation->world->database, destination),
              victim));
      game_object_set_exits(evaluation->world->database, victim, destination);

      if (!is_quiet(evaluation->world->database, player))
        notify_quiet(evaluation, player, "Exit teleported.");
    } else if (move_via_teleport(evaluation, victim, destination, cause,
                                 hush)) {
      if (player != victim && !is_quiet(evaluation->world->database, player))
        notify_quiet(evaluation, player, "Teleported.");
    }
  } else if (is_exit(evaluation->world->database, destination)) {
    if (game_object_exits(evaluation->world->database, destination) ==
        game_object_location(evaluation->world->database, victim)) {
      move_exit(evaluation, victim, destination, 0, "You can't go that way.",
                0);
    } else {
      notify_quiet(evaluation, player, "I can't find that exit.");
    }
  }
}

/**
 * Interlude to do_force for the # command
 */
void do_force_prefixed(CommandInvocation *invocation) {
  char *command = invocation->first;
  char *cp;

  cp = parse_to(invocation->context->server->configuration, &command, ' ', 0);
  if (!command)
    return;
  while (*command && isspace(*command))
    command++;
  if (*command) {
    CommandInvocation force_invocation = *invocation;

    force_invocation.first = cp;
    force_invocation.second = command;
    do_force(&force_invocation);
  }
}

/**
 * Force an object to do something.
 */
void do_force(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *what = invocation->first;
  char *command = invocation->second;
  char **args = invocation->vector;
  int nargs = invocation->vector_count;
  DbRef victim;

  if ((victim = match_controlled(&invocation->context->match, player, what)) ==
      NOTHING)
    return;

  /*
   * force victim to do command
   */

  wait_que(invocation->context->server->commands, victim, player, 0, NOTHING, 0,
           command, args, nargs, invocation->context->evaluation.registers);
}

void do_newpassword(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *password = invocation->second;
  ServerConfiguration *configuration =
      invocation->context->server->configuration;
  DbRef victim;
  char hashed_password[crypto_pwhash_STRBYTES];
  char *buf;

  if ((victim = lookup_player(invocation->context->world, player, name, 0)) ==
      NOTHING) {
    notify_quiet(evaluation, player, "No such player.");
    return;
  }
  if (*password != '\0' && !ok_password(configuration, password)) {

    /*
     * Can set null passwords, but not bad passwords
     */
    notify_quiet(evaluation, player, "Bad password");
    return;
  }
  if (is_god(evaluation->world->database, victim)) {
    notify_quiet(evaluation, player,
                 "You cannot change that player's password.");
    return;
  }
  if (!password_hash(configuration, password, hashed_password)) {
    notify_quiet(evaluation, player, "Unable to change password.");
    return;
  }
  object_password_set(evaluation->world->database, victim, hashed_password);
  sodium_memzero(hashed_password, sizeof(hashed_password));
  STARTLOG(&evaluation->server->log, LOG_WIZARD, "WIZ", "PASS") {
    log_name(&evaluation->server->log, player);
    log_text(" changed the password of ");
    log_name(&evaluation->server->log, victim);
    ENDLOG(&evaluation->server->log);
  }
  buf = alloc_lbuf("do_newpassword");
  notify_quiet(evaluation, player, "Password changed.");
  snprintf(buf, LBUF_SIZE, "Your password has been changed by %s.",
           game_object_name(invocation->context->world->database, player));
  notify_quiet(evaluation, victim, buf);
  free_lbuf(buf);
}

void do_boot(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  DbRef victim;
  char *buf, *bp;
  int count;

  if (!is_wizard(evaluation->world->database, player)) {
    notify(evaluation, player, "Permission denied.");
    return;
  }
  if (key & BOOT_PORT) {
    if (is_number(name)) {
      victim = atoi(name);
    } else {
      notify_quiet(evaluation, player, "That's not a number!");
      return;
    }
    STARTLOG(&evaluation->server->log, LOG_WIZARD, "WIZ", "BOOT") {
      buf = alloc_sbuf("do_boot.port");
      snprintf(buf, SBUF_SIZE, "Port %ld", victim);
      log_text(buf);
      log_text(" was @booted by ");
      log_name(&evaluation->server->log, player);
      free_sbuf(buf);
      ENDLOG(&evaluation->server->log);
    }
  } else {
    init_match(&invocation->context->match, player, name, TYPE_PLAYER);
    match_neighbor(&invocation->context->match);
    match_absolute(&invocation->context->match);
    match_player(&invocation->context->match);
    if ((victim = noisy_match_result(&invocation->context->match)) == NOTHING)
      return;

    if (is_god(evaluation->world->database, victim)) {
      notify_quiet(evaluation, player, "You cannot boot that player!");
      return;
    }
    if ((!is_player(evaluation->world->database, victim) &&
         !is_god(evaluation->world->database, player)) ||
        (player == victim)) {
      notify_quiet(evaluation, player, "You can only boot off other players!");
      return;
    }
    STARTLOG(&evaluation->server->log, LOG_WIZARD, "WIZ", "BOOT") {
      log_name_and_loc(&evaluation->server->log, victim);
      log_text(" was @booted by ");
      log_name(&evaluation->server->log, player);
      ENDLOG(&evaluation->server->log);
    }
    notify_quiet(evaluation, player,
                 tprintf("You booted %s off!",
                         game_object_name(invocation->context->world->database,
                                          victim)));
  }
  if (key & BOOT_QUIET) {
    buf = nullptr;
  } else {
    bp = buf = alloc_lbuf("do_boot.msg");
    safe_str(game_object_name(invocation->context->world->database, player),
             buf, &bp);
    safe_str(" gently shows you the door.", buf, &bp);
    *bp = '\0';
  }

  if (key & BOOT_PORT)
    count = boot_by_port(invocation->context->server->descriptors, (int)victim,
                         !is_god(evaluation->world->database, player), buf);
  else
    count = boot_off(invocation->context->server->descriptors, victim, buf);
  notify_quiet(
      evaluation, player,
      tprintf("%d connection%s closed.", count, (count == 1 ? "" : "s")));
  if (buf)
    free_lbuf(buf);
}

/**
 * Reduce the wealth of anyone over a specified amount.
 */
/**
 * Chop off a contents or exits chain after the named item.
 */
void do_cut(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *thing = invocation->first;
  DbRef object;

  object = match_controlled(&invocation->context->match, player, thing);
  switch (object) {
  case NOTHING:
    notify_quiet(evaluation, player, "No match.");
    break;
  case AMBIGUOUS:
    notify_quiet(evaluation, player, "I don't know which one");
    break;
  default:
    game_object_set_next(evaluation->world->database, object, NOTHING);
    notify_quiet(evaluation, player, "Cut.");
  }
}

typedef enum GlobalControl {
  GLOBAL_CONTROL_CHECKPOINTING,
  GLOBAL_CONTROL_CLEANING,
  GLOBAL_CONTROL_DEQUEUEING,
  GLOBAL_CONTROL_IDLE_CHECKING,
  GLOBAL_CONTROL_INTERPRET,
  GLOBAL_CONTROL_LOGINS,
  GLOBAL_CONTROL_EVENT_CHECKING,
} GlobalControl;

static const NameTable enable_names[] = {
    {"checkpointing", 2, CA_PUBLIC, GLOBAL_CONTROL_CHECKPOINTING},
    {"cleaning", 2, CA_PUBLIC, GLOBAL_CONTROL_CLEANING},
    {"dequeueing", 1, CA_PUBLIC, GLOBAL_CONTROL_DEQUEUEING},
    {"idlechecking", 2, CA_PUBLIC, GLOBAL_CONTROL_IDLE_CHECKING},
    {"interpret", 2, CA_PUBLIC, GLOBAL_CONTROL_INTERPRET},
    {"logins", 3, CA_PUBLIC, GLOBAL_CONTROL_LOGINS},
    {"eventchecking", 2, CA_PUBLIC, GLOBAL_CONTROL_EVENT_CHECKING},
    {nullptr, 0, 0, 0}};

static bool *global_control_value(ServerConfiguration *configuration,
                                  int control) {
  switch (control) {
  case GLOBAL_CONTROL_CHECKPOINTING:
    return &configuration->is_checkpointing_enabled;
  case GLOBAL_CONTROL_CLEANING:
    return &configuration->is_db_check_enabled;
  case GLOBAL_CONTROL_DEQUEUEING:
    return &configuration->is_dequeue_enabled;
  case GLOBAL_CONTROL_IDLE_CHECKING:
    return &configuration->is_idle_check_enabled;
  case GLOBAL_CONTROL_INTERPRET:
    return &configuration->is_interpreter_enabled;
  case GLOBAL_CONTROL_LOGINS:
    return &configuration->is_login_enabled;
  case GLOBAL_CONTROL_EVENT_CHECKING:
    return &configuration->is_event_check_enabled;
  default:
    return nullptr;
  }
}

void list_global_controls(EvaluationContext *evaluation,
                          ServerConfiguration *configuration, DbRef player) {
  char *buf = alloc_lbuf("list_global_controls");
  char *bp = buf;

  safe_str("Global parameters:", buf, &bp);
  for (const NameTable *control = enable_names; control->name; control++) {
    const bool *is_enabled = global_control_value(configuration, control->flag);

    safe_chr(' ', buf, &bp);
    safe_str(control->name, buf, &bp);
    safe_str(*is_enabled ? "...enabled" : "...disabled", buf, &bp);
    if ((control + 1)->name)
      safe_chr(';', buf, &bp);
  }
  *bp = '\0';
  notify(evaluation, player, buf);
  free_lbuf(buf);
}

void do_global(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *flag = invocation->first;
  int control;
  bool *is_enabled;

  /*
   * Set or clear the indicated flag
   */

  control = name_table_search(&invocation->context->server->database,
                              invocation->context->server->configuration,
                              player, enable_names, flag);
  if (control < 0) {
    notify_quiet(evaluation, player, "I don't know about that flag.");
    return;
  }

  is_enabled =
      global_control_value(invocation->context->server->configuration, control);
  if (key == GLOB_ENABLE) {
    *is_enabled = true;
    if (!is_quiet(evaluation->world->database, player))
      notify_quiet(evaluation, player, "Enabled.");
  } else if (key == GLOB_DISABLE) {
    *is_enabled = false;
    if (!is_quiet(evaluation->world->database, player))
      notify_quiet(evaluation, player, "Disabled.");
  } else {
    notify_quiet(evaluation, player, "Illegal combination of switches.");
  }
}
