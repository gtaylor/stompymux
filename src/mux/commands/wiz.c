/*
 * wiz.c -- Wizard-only commands
 */

#include <crypto_pwhash.h>
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <utils.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_keys.h"
#include "mux/commands/command_parser.h"
#include "mux/commands/command_queue.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/connection_events.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/name_table.h"
#include "mux/support/password.h"
#include "mux/support/stringutil.h"
#include "mux/support/validation.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/object_list.h"
#include "mux/world/object_set.h"
#include "mux/world/player.h"

void do_teleport(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  int key = invocation->key;
  char *arg1 = invocation->first;
  char *arg2 = invocation->second;
  ServerConfiguration *configuration =
      invocation->context->world->configuration;
  DbRef victim;
  DbRef destination;
  DbRef exitloc;
  char *to;
  int hush = 0;
  LuaLockInvocation lock;

  /*
   * get victim
   */

  if (*arg2 == '\0') {
    victim = player;
    to = arg1;
  } else {
    init_match(&invocation->context->match, player, arg1, OBJECT_TYPE_NOTYPE);
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
      typeof_obj(evaluation->world->database, victim) != OBJECT_TYPE_EXIT) {
    notify_checked(evaluation, player, player, "You can't teleport that.",
                   MSG_ME);
    return;
  }
  /*
   * Fail if we don't control the victim or the victim's location
   */

  if (!is_controls(evaluation->world->database, player, victim) &&
      !is_controls(evaluation->world->database, player,
                   game_object_location(evaluation->world->database, victim)) &&
      !is_wizard(evaluation->world->database, player)) {
    notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    return;
  }
  /*
   * Check for teleporting home
   * Also, can't teleport exits 'home'
   */

  if (!string_compare(configuration, to, "home") &&
      typeof_obj(evaluation->world->database, victim) != OBJECT_TYPE_EXIT) {
    (void)move_via_teleport(&(ObjectMovementRequest){.evaluation = evaluation,
                                                     .object = victim,
                                                     .destination = HOME,
                                                     .cause = cause});
    return;
  }
  /*
   * Find out where to send the victim
   */

  init_match(&invocation->context->match, player, to, OBJECT_TYPE_NOTYPE);
  match_everything(&invocation->context->match, 0);
  destination = match_result(&invocation->context->match);

  switch (destination) {
  case NOTHING:
    notify_checked(evaluation, player, player, "No match.", MSG_ME);
    return;
  case AMBIGUOUS:
    notify_checked(evaluation, player, player,
                   "I don't know which destination you mean!", MSG_ME);
    return;
  default:
    if (victim == destination) {
      notify_checked(evaluation, player, player, "Bad destination.", MSG_ME);
      return;
    }
  }

  if (has_contents(evaluation->world->database, destination)) {
    LuaLockResult *result = checked_storage_allocate(sizeof(*result));

    /*
     * You must control the destination and pass its TELEPORT lock.
     */

    bool permitted =
        (is_controls(evaluation->world->database, player, destination) ||
         is_wizard(evaluation->world->database, player)) != 0;

    if (permitted) {
      permitted = lock_test(evaluation, victim, player, player, destination,
                            LUA_LOCK_TELEPORT, LUA_LOCK_OPERATION_TELEPORT,
                            false, &lock, result);
    } else {
      lock = (LuaLockInvocation){
          .type = LUA_LOCK_TELEPORT,
          .operation = LUA_LOCK_OPERATION_TELEPORT,
          .descriptor = invocation->context->descriptor,
          .object = destination,
          .enactor = victim,
          .cause = player,
          .subject = player,
      };
      *result = (LuaLockResult){};
    }
    if (!permitted) {

      /*
       * Nope, report failure
       */

      if (player != victim)
        notify_checked(evaluation, player, player, "Permission denied.",
                       MSG_ME);
      notify_lock_failure(&(LockFailureNotification){
          .evaluation = evaluation,
          .invocation = &lock,
          .result = result,
          .enactor_default = "You can't teleport there!",
          .event = LUA_EVENT_TELEPORT_DESTINATION_FAIL});
      free_buf(result);
      return;
    }
    /*
     * We're OK, do the teleport
     */

    if ((key & TELEPORT_QUIET) || is_dark(evaluation->world->database, victim))
      hush = HUSH_ENTER | HUSH_LEAVE;

    if (typeof_obj(evaluation->world->database, victim) == OBJECT_TYPE_EXIT) {
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

      notify_checked(evaluation, player, player, "Exit teleported.", MSG_ME);
    } else if (move_via_teleport(
                   &(ObjectMovementRequest){.evaluation = evaluation,
                                            .object = victim,
                                            .destination = destination,
                                            .cause = cause,
                                            .hush = hush})) {
      if (player != victim)
        notify_checked(evaluation, player, player, "Teleported.", MSG_ME);
    }
    free_buf(result);
  } else if (is_exit(evaluation->world->database, destination)) {
    if (game_object_exits(evaluation->world->database, destination) ==
        game_object_location(evaluation->world->database, victim)) {
      move_exit(evaluation, victim, destination, "You can't go that way.", 0);
    } else {
      notify_checked(evaluation, player, player, "I can't find that exit.",
                     MSG_ME);
    }
  }
}

/**
 * Interlude to do_force for the # command
 */
void do_force_prefixed(CommandInvocation *invocation) {
  char *command = invocation->first;
  char *cp;

  cp = parse_to(&(CommandParseRequest){
      .configuration = invocation->context->world->configuration,
      .source = &command,
      .delimiter = ' '});
  if (!command)
    return;
  const size_t COMMAND_LENGTH = strlen(command);
  size_t offset = 0;

  while (offset < COMMAND_LENGTH &&
         (isspace)((unsigned char)*(const char *)checked_storage_at_const(
             command, COMMAND_LENGTH + 1, sizeof(char), offset)))
    offset++;
  command = checked_mutable_string_suffix(command, offset);
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
  DbRef victim;

  victim = match_controlled(&invocation->context->match, player, what);
  if (victim == NOTHING)
    return;

  /*
   * force victim to do command
   */

  wait_que(
      &(QueuedCommandRequest){.queue = invocation->context->runtime->commands,
                              .player = victim,
                              .cause = player,
                              .command = command});
}

void do_newpassword(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *password = invocation->second;
  ServerConfiguration *configuration =
      invocation->context->world->configuration;
  DbRef victim;
  char hashed_password[crypto_pwhash_STRBYTES];
  char *buf;

  victim = lookup_player(invocation->context->world, player, name, 0);
  if (victim == NOTHING) {
    notify_checked(evaluation, player, player, "No such player.", MSG_ME);
    return;
  }
  if (*password != '\0' && !ok_password(configuration, password)) {

    /*
     * Can set null passwords, but not bad passwords
     */
    notify_checked(evaluation, player, player, "Bad password", MSG_ME);
    return;
  }
  if (is_god(evaluation->world->database, victim)) {
    notify_checked(evaluation, player, player,
                   "You cannot change that player's password.", MSG_ME);
    return;
  }
  if (!password_hash(configuration, password, hashed_password)) {
    notify_checked(evaluation, player, player, "Unable to change password.",
                   MSG_ME);
    return;
  }
  if (!object_password_set(evaluation->world->database, victim,
                           hashed_password)) {
    sodium_memzero(hashed_password, sizeof(hashed_password));
    notify_checked(evaluation, player, player, "Unable to change password.",
                   MSG_ME);
    return;
  }
  sodium_memzero(hashed_password, sizeof(hashed_password));
  STARTLOG(evaluation->log, LOG_WIZARD, "WIZ", "PASS") {
    log_name(evaluation->log, player);
    log_text(" changed the password of ");
    log_name(evaluation->log, victim);
    ENDLOG(evaluation->log);
  }
  buf = alloc_lbuf("do_newpassword");
  notify_checked(evaluation, player, player, "Password changed.", MSG_ME);
  (void)snprintf(
      buf, LBUF_SIZE, "Your password has been changed by %s.",
      game_object_name(invocation->context->world->database, player));
  notify_checked(evaluation, victim, victim, buf, MSG_ME);
  free_buf(buf);
}

void do_boot(CommandInvocation *invocation) {
  char message_buffer[LBUF_SIZE];
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  DbRef victim;
  char *buf;
  char *bp;
  int count;

  if (!is_wizard(evaluation->world->database, player)) {
    notify_checked(evaluation, player, player, "Permission denied.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  if (key & BOOT_PORT) {
    if (parse_long_checked(name, &victim)) {
    } else {
      notify_checked(evaluation, player, player, "That's not a number!",
                     MSG_ME);
      return;
    }
    STARTLOG(evaluation->log, LOG_WIZARD, "WIZ", "BOOT") {
      char port[SBUF_SIZE];
      (void)snprintf(port, sizeof(port), "Port %ld", victim);
      log_text(port);
      log_text(" was @booted by ");
      log_name(evaluation->log, player);
      ENDLOG(evaluation->log);
    }
  } else {
    init_match(&invocation->context->match, player, name, OBJECT_TYPE_PLAYER);
    match_neighbor(&invocation->context->match);
    match_absolute(&invocation->context->match);
    match_player(&invocation->context->match);
    victim = noisy_match_result(&invocation->context->match);
    if (victim == NOTHING)
      return;

    if (is_god(evaluation->world->database, victim)) {
      notify_checked(evaluation, player, player, "You cannot boot that player!",
                     MSG_ME);
      return;
    }
    if ((!is_player(evaluation->world->database, victim) &&
         !is_god(evaluation->world->database, player)) ||
        (player == victim)) {
      notify_checked(evaluation, player, player,
                     "You can only boot off other players!", MSG_ME);
      return;
    }
    STARTLOG(evaluation->log, LOG_WIZARD, "WIZ", "BOOT") {
      log_name_and_loc(evaluation->log, victim);
      log_text(" was @booted by ");
      log_name(evaluation->log, player);
      ENDLOG(evaluation->log);
    }
    (void)snprintf(
        message_buffer, sizeof(message_buffer), "You booted %s off!",
        game_object_name(invocation->context->world->database, victim));
    notify_checked(evaluation, player, player, message_buffer, MSG_ME);
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
    count = boot_by_port(invocation->context->runtime->descriptors, (int)victim,
                         !is_god(evaluation->world->database, player), buf);
  else
    count = boot_off(invocation->context->runtime->descriptors, victim, buf);
  (void)snprintf(message_buffer, sizeof(message_buffer),
                 "%d connection%s closed.", count, (count == 1 ? "" : "s"));
  notify_checked(evaluation, player, player, message_buffer, MSG_ME);
  if (buf)
    free_buf(buf);
}

typedef enum GlobalControl : int {
  GLOBAL_CONTROL_CHECKPOINTING,
  GLOBAL_CONTROL_CLEANING,
  GLOBAL_CONTROL_IDLE_CHECKING,
  GLOBAL_CONTROL_QUEUEING,
  GLOBAL_CONTROL_LOGINS,
} GlobalControl;

static const NameTable ENABLE_NAMES[] = {
    {"checkpointing", 2, CA_PUBLIC, GLOBAL_CONTROL_CHECKPOINTING},
    {"cleaning", 2, CA_PUBLIC, GLOBAL_CONTROL_CLEANING},
    {"idlechecking", 2, CA_PUBLIC, GLOBAL_CONTROL_IDLE_CHECKING},
    {"queueing", 2, CA_PUBLIC, GLOBAL_CONTROL_QUEUEING},
    {"logins", 3, CA_PUBLIC, GLOBAL_CONTROL_LOGINS},
    {nullptr, 0, 0, 0}};

static bool *global_control_value(ServerConfiguration *configuration,
                                  int control) {
  switch (control) {
  case GLOBAL_CONTROL_CHECKPOINTING:
    return &configuration->is_checkpointing_enabled;
  case GLOBAL_CONTROL_CLEANING:
    return &configuration->is_db_check_enabled;
  case GLOBAL_CONTROL_IDLE_CHECKING:
    return &configuration->is_idle_check_enabled;
  case GLOBAL_CONTROL_QUEUEING:
    return &configuration->is_command_queue_enabled;
  case GLOBAL_CONTROL_LOGINS:
    return &configuration->is_login_enabled;
  default:
    return nullptr;
  }
}

void list_global_controls(EvaluationContext *evaluation,
                          ServerConfiguration *configuration, DbRef player) {
  char *buf = alloc_lbuf("list_global_controls");
  char *bp = buf;
  constexpr size_t CONTROL_COUNT =
      (sizeof(ENABLE_NAMES) / sizeof(ENABLE_NAMES[0])) - 1;

  safe_str("Global parameters:", buf, &bp);
  for (size_t index = 0; index < CONTROL_COUNT; index++) {
    const NameTable *control = checked_storage_at_const(
        ENABLE_NAMES, CONTROL_COUNT, sizeof(*ENABLE_NAMES), index);
    const bool *is_enabled = global_control_value(configuration, control->flag);

    safe_chr(' ', buf, &bp);
    safe_str(control->name, buf, &bp);
    safe_str(*is_enabled ? "...enabled" : "...disabled", buf, &bp);
    if (index + 1 < CONTROL_COUNT)
      safe_chr(';', buf, &bp);
  }
  *bp = '\0';
  notify_checked(evaluation, player, player, buf, MSG_ME_ALL | MSG_F_DOWN);
  free_buf(buf);
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

  control = name_table_search(invocation->context->world->database,
                              invocation->context->world->configuration, player,
                              ENABLE_NAMES, flag);
  if (control < 0) {
    notify_checked(evaluation, player, player, "I don't know about that flag.",
                   MSG_ME);
    return;
  }

  is_enabled =
      global_control_value(invocation->context->world->configuration, control);
  if (key == GLOB_ENABLE) {
    *is_enabled = true;
    notify_checked(evaluation, player, player, "Enabled.", MSG_ME);
  } else if (key == GLOB_DISABLE) {
    *is_enabled = false;
    notify_checked(evaluation, player, player, "Disabled.", MSG_ME);
  } else {
    notify_checked(evaluation, player, player,
                   "Illegal combination of switches.", MSG_ME);
  }
}
