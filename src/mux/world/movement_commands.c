/*
 * move.c -- Routines for moving about
 */

#include <stdio.h>
#include <string.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/movement_commands.h"

/*
 * ---------------------------------------------------------------------------
 * * process_leave_loc: Generate messages and actions resulting from leaving
 * * a place.
 */

void move_command(EvaluationContext *evaluation, DbRef player, DbRef cause,
                  int key, char *direction) {
  DbRef exit, loc;
  int i, quiet;
  const ServerConfiguration *configuration = evaluation->world->configuration;
  MatchContext *match = &evaluation->command->match;

  if (!string_compare(configuration, direction, "home")) { /*
                                                            * go home w/o stuff
                                                            */

    if ((loc = game_object_location(evaluation->world->database, player)) !=
            NOTHING &&
        !is_dark(evaluation->world->database, player) &&
        !is_dark(evaluation->world->database, loc)) {

      /*
       * tell all
       */
      char buffer[MBUF_SIZE];
      memset(buffer, 0, MBUF_SIZE);
      (void)snprintf(buffer, MBUF_SIZE - 1, "%s goes home.",
                     game_object_name(evaluation->world->database, player));
      notify_except(evaluation, loc, player, player, buffer);
    }
    /*
     * give the player the messages
     */

    for (i = 0; i < 3; i++)
      notify_checked(evaluation, player, player,
                     "There's no place like home...", MSG_ME_ALL | MSG_F_DOWN);
    move_via_generic(evaluation, player, HOME, NOTHING, 0);
    return;
  }
  /*
   * find the exit
   */

  init_match_check_keys(match, player, direction, OBJECT_TYPE_EXIT);
  match_exit(match);
  exit = match_result(match);
  switch (exit) {
  case NOTHING: /*
                 * try to force the object
                 */
    notify_checked(evaluation, player, player, "You can't go that way.",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  case AMBIGUOUS:
    notify_checked(evaluation, player, player,
                   "I don't know which way you mean!", MSG_ME_ALL | MSG_F_DOWN);
    break;
  default:
    quiet = 0;
    if ((key & MOVE_QUIET) &&
        is_controls(evaluation->world->database, player, exit))
      quiet = HUSH_EXIT;
    move_exit(evaluation, player, exit, "You can't go that way.", quiet);
  }
}

void do_move(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  move_command(evaluation, invocation->player, invocation->cause,
               invocation->key, invocation->first);
}

/*
 * ---------------------------------------------------------------------------
 * * do_get: Get an object.
 */

void do_enter_internal(EvaluationContext *evaluation, DbRef player, DbRef thing,
                       int quiet) {
  DbRef loc = game_object_location(evaluation->world->database, player);
  int oattr;
  LuaLockInvocation lock;
  LuaLockResult result;

  if (player == thing) {
    notify_checked(evaluation, player, player, "You can't enter yourself!",
                   MSG_ME_ALL | MSG_F_DOWN);
  } else if (lock_test(evaluation, player, player, player, thing,
                       LUA_LOCK_ENTER, LUA_LOCK_OPERATION_ENTER, quiet, &lock,
                       &result) &&
             lock_test(evaluation, player, player, player, loc, LUA_LOCK_LEAVE,
                       LUA_LOCK_OPERATION_ENTER, quiet, &lock, &result)) {
    oattr = quiet ? HUSH_ENTER : 0;
    move_via_generic(evaluation, player, thing, NOTHING, oattr);
  } else {
    notify_lock_failure(evaluation, &lock, &result, "You can't enter that.",
                        nullptr, LUA_EVENT_ENTER_FAIL);
  }
}

void do_enter(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *what = invocation->first;
  DbRef thing;
  int quiet;

  MatchContext *match = &invocation->context->match;
  init_match(match, player, what, OBJECT_TYPE_THING);
  match_neighbor(match);

  if ((thing = noisy_match_result(match)) == NOTHING)
    return;

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_PLAYER:
  case OBJECT_TYPE_THING:
    quiet = 0;
    if ((key & MOVE_QUIET) &&
        is_controls(evaluation->world->database, player, thing))
      quiet = 1;
    do_enter_internal(evaluation, player, thing, quiet);
    break;
  default:
    notify_checked(evaluation, player, player, "Permission denied.",
                   MSG_ME_ALL | MSG_F_DOWN);
  }
  return;
}

void do_leave(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  DbRef loc;
  int quiet;
  LuaLockInvocation lock;
  LuaLockResult result;

  loc = game_object_location(evaluation->world->database, player);

  if (!is_good_obj(evaluation->world->database, loc) ||
      is_room(evaluation->world->database, loc) ||
      is_going(evaluation->world->database, loc)) {
    notify_checked(evaluation, player, player, "You can't leave.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  quiet = 0;
  if ((key & MOVE_QUIET) &&
      is_controls(evaluation->world->database, player, loc))
    quiet = HUSH_LEAVE;
  if (lock_test(evaluation, player, invocation->cause, player, loc,
                LUA_LOCK_LEAVE, LUA_LOCK_OPERATION_LEAVE, quiet, &lock,
                &result) &&
      lock_test(evaluation, player, invocation->cause, player,
                game_object_location(evaluation->world->database, loc),
                LUA_LOCK_ENTER, LUA_LOCK_OPERATION_LEAVE, quiet, &lock,
                &result)) {
    move_via_generic(evaluation, player,
                     game_object_location(evaluation->world->database, loc),
                     NOTHING, quiet);
  } else {
    notify_lock_failure(evaluation, &lock, &result, "You can't leave.", nullptr,
                        LUA_EVENT_LEAVE_FAIL);
  }
}
