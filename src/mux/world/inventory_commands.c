/*
 * move.c -- Routines for moving about
 */

#include "mux/world/inventory_commands.h"
#include "mux/commands/action_messages.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/move_internal.h"
#include "mux/world/object_list.h"

/*
 * ---------------------------------------------------------------------------
 * * process_leave_loc: Generate messages and actions resulting from leaving
 * * a place.
 */

void do_get(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *what = invocation->first;
  DbRef thing;
  DbRef playerloc;
  DbRef thingloc;
  const char *failmsg;
  int quiet;
  LuaLockInvocation lock;

  playerloc = game_object_location(evaluation->world->database, player);
  if (!is_good_obj(evaluation->world->database, playerloc))
    return;

  /*
   * Look for the thing locally
   */

  MatchContext *match = &invocation->context->match;
  init_match_check_keys(match, player, what, OBJECT_TYPE_THING);
  match_neighbor(match);
  match_exit(match);
  thing = match_result(match);

  /*
   * Look for the thing in other people's inventories
   */

  if (!is_good_obj(evaluation->world->database, thing))
    thing = match_status(evaluation, player,
                         match_possessed(match, player, player, what, thing));
  if (!is_good_obj(evaluation->world->database, thing))
    return;

  /*
   * If we found it, get it
   */

  quiet = 0;
  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_PLAYER:
  case OBJECT_TYPE_THING:
    /*
     * You can't take what you already have
     */

    thingloc = game_object_location(evaluation->world->database, thing);
    if (thingloc == player) {
      notify_checked(evaluation, player, player, "You already have that!",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    }
    if ((key & GET_QUIET) &&
        is_controls(evaluation->world->database, player, thing))
      quiet = 1;

    if (thing == player) {
      notify_checked(evaluation, player, player, "You cannot get yourself!",
                     MSG_ME_ALL | MSG_F_DOWN);
    } else {
      LuaLockResult *result = checked_storage_allocate(sizeof(*result));
      if (lock_test(evaluation, player, invocation->cause, player, thing,
                    LUA_LOCK_DEFAULT, LUA_LOCK_OPERATION_TAKE, quiet != 0,
                    &lock, result)) {
        if (thingloc !=
            game_object_location(evaluation->world->database, player)) {
          notify_printf(evaluation, thingloc, "%s was taken from you.",
                        game_object_name(evaluation->world->database, thing));
        }
        move_via_generic(&(ObjectMovementRequest){.evaluation = evaluation,
                                                  .object = thing,
                                                  .destination = player,
                                                  .cause = player});
        notify_checked(evaluation, thing, thing, "Taken.",
                       MSG_ME_ALL | MSG_F_DOWN);
        notify_action(
            evaluation,
            &(ActionMessageInvocation){
                .message = {.type = LUA_MESSAGE_SUCCESS,
                            .operation = LUA_MESSAGE_OPERATION_TAKE,
                            .descriptor = invocation->context->descriptor,
                            .object = thing,
                            .enactor = player,
                            .cause = invocation->cause,
                            .source = thingloc,
                            .destination = player,
                            .silent = quiet != 0},
                .enactor_default = "Taken.",
                .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_SUCCESS});
      } else {
        if (thingloc !=
            game_object_location(evaluation->world->database, player))
          failmsg = "You can't take that from there.";
        else
          failmsg = "You can't pick that up.";
        notify_lock_failure(
            &(LockFailureNotification){.evaluation = evaluation,
                                       .invocation = &lock,
                                       .result = result,
                                       .enactor_default = failmsg,
                                       .event = LUA_EVENT_FAIL});
      }
      free_buf(result);
    }
    break;
  case OBJECT_TYPE_EXIT:
    /*
     * You can't take what you already have
     */

    thingloc = game_object_exits(evaluation->world->database, thing);
    if (thingloc == player) {
      notify_checked(evaluation, player, player, "You already have that!",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    }
    /*
     * You must control either the exit or the location
     */

    playerloc = game_object_location(evaluation->world->database, player);
    if (!is_controls(evaluation->world->database, player, thing) &&
        !is_controls(evaluation->world->database, player, playerloc)) {
      notify_checked(evaluation, player, player, "Permission denied.",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    }
    /*
     * Do it
     */

    game_object_set_exits(
        evaluation->world->database, thingloc,
        remove_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, thingloc),
                     thing));
    game_object_set_exits(
        evaluation->world->database, player,
        insert_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, player),
                     thing));
    game_object_set_exits(evaluation->world->database, thing, player);
    notify_checked(evaluation, player, player, "Exit taken.",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  default:
    notify_checked(evaluation, player, player, "You can't take that!",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_drop: Drop an object.
 */

void do_drop(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  DbRef loc;
  DbRef exitloc;
  DbRef thing;
  char *buf;
  char *bp;
  int quiet;
  LuaLockInvocation lock;

  loc = game_object_location(evaluation->world->database, player);
  if (!is_good_obj(evaluation->world->database, loc))
    return;

  MatchContext *match = &invocation->context->match;
  init_match(match, player, name, OBJECT_TYPE_THING);
  match_possession(match);
  match_carried_exit(match);

  switch (thing = match_result(match)) {
  case NOTHING:
    notify_checked(evaluation, player, player, "You don't have that!",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  case AMBIGUOUS:
    notify_checked(evaluation, player, player, "I don't know which you mean!",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  default:
    break;
  }

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_THING:
  case OBJECT_TYPE_PLAYER:

    /*
     * You have to be carrying it
     */

    if ((game_object_location(evaluation->world->database, thing) != player) &&
        !is_wizard(evaluation->world->database, player)) {
      notify_checked(evaluation, player, player, "You can't drop that.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
    LuaLockResult *result = checked_storage_allocate(sizeof(*result));
    if (!lock_test(evaluation, player, invocation->cause, player, thing,
                   LUA_LOCK_DROP, LUA_LOCK_OPERATION_DROP, false, &lock,
                   result)) {
      notify_lock_failure(
          &(LockFailureNotification){.evaluation = evaluation,
                                     .invocation = &lock,
                                     .result = result,
                                     .enactor_default = "You can't drop that.",
                                     .event = LUA_EVENT_DROP_FAIL});
      free_buf(result);
      return;
    }
    /*
     * Move it
     */

    move_via_generic(&(ObjectMovementRequest){
        .evaluation = evaluation,
        .object = thing,
        .destination =
            game_object_location(evaluation->world->database, player),
        .cause = player});
    notify_checked(evaluation, thing, thing, "Dropped.",
                   MSG_ME_ALL | MSG_F_DOWN);
    quiet = 0;
    if ((key & DROP_QUIET) &&
        is_controls(evaluation->world->database, player, thing))
      quiet = 1;
    bp = buf = alloc_lbuf("do_drop.notify_action");
    safe_tprintf_str(buf, &bp, "dropped %s.",
                     game_object_name(evaluation->world->database, thing));
    notify_action(evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_DROP,
                                  .operation = LUA_MESSAGE_OPERATION_DROP,
                                  .descriptor = invocation->context->descriptor,
                                  .object = thing,
                                  .enactor = player,
                                  .cause = invocation->cause,
                                  .source = player,
                                  .destination = loc,
                                  .silent = quiet != 0},
                      .enactor_default = "Dropped.",
                      .other_default = buf,
                      .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_DROP});
    free_buf(buf);

    /*
     * Process droptos
     */

    process_dropped_dropto(evaluation, thing, player);
    free_buf(result);

    break;
  case OBJECT_TYPE_EXIT:

    /*
     * You have to be carrying it
     */

    if ((game_object_exits(evaluation->world->database, thing) != player) &&
        !is_wizard(evaluation->world->database, player)) {
      notify_checked(evaluation, player, player, "You can't drop that.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
    if (!is_controls(evaluation->world->database, player, loc)) {
      notify_checked(evaluation, player, player, "Permission denied.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    }
    /*
     * Do it
     */

    exitloc = game_object_exits(evaluation->world->database, thing);
    game_object_set_exits(
        evaluation->world->database, exitloc,
        remove_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, exitloc),
                     thing));
    game_object_set_exits(
        evaluation->world->database, loc,
        insert_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, loc),
                     thing));
    game_object_set_exits(evaluation->world->database, thing, loc);

    notify_checked(evaluation, player, player, "Exit dropped.",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  default:
    notify_checked(evaluation, player, player, "You can't drop that.",
                   MSG_ME_ALL | MSG_F_DOWN);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_enter, do_leave: The enter and leave commands.
 */
