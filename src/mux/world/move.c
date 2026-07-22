/*
 * move.c -- Routines for moving about
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/command_invocation.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/world_context.h"

/*
 * ---------------------------------------------------------------------------
 * * process_leave_loc: Generate messages and actions resulting from leaving
 * * a place.
 */

static void process_leave_loc(EvaluationContext *evaluation, DbRef thing,
                              DbRef dest, DbRef cause, int canhear, int hush) {
  DbRef loc;
  int quiet;

  loc = game_object_location(evaluation->world->database, thing);
  if ((loc == NOTHING) || (loc == dest))
    return;

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);

  /*
   * Run the LEAVE attributes in the current room if we meet any of * *
   *
   * *  * * following criteria: * - The current room has wizard privs.
   * * - * * * Neither the current room nor the moving object are dark.
   * * - The * *  * moving object can hear and does not hav wizard
   * privs. * EXCEPT  * if * * we were called with the HUSH_LEAVE key.
   */

  quiet = (!(is_wizard(evaluation->world->database, loc) ||
             (!is_dark(evaluation->world->database, thing) &&
              !is_dark(evaluation->world->database, loc)) ||
             (canhear && !(is_wizard(evaluation->world->database, thing) &&
                           is_dark(evaluation->world->database, thing))))) ||
          (hush & HUSH_LEAVE);
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_LEAVE,
                                .operation = LUA_MESSAGE_OPERATION_MOVE,
                                .object = loc,
                                .enactor = thing,
                                .cause = cause,
                                .source = loc,
                                .destination = dest,
                                .silent = quiet},
                    .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_LEAVE});

  /*
   * Do OXENTER for receiving room
   */

  if ((dest != NOTHING) && !quiet)
    notify_action(evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_ENTER_SOURCE,
                                  .operation = LUA_MESSAGE_OPERATION_MOVE,
                                  .object = dest,
                                  .enactor = thing,
                                  .cause = cause,
                                  .source = loc,
                                  .destination = dest}});

  /*
   * Display the 'has left' message if we meet any of the following * *
   *
   * *  * * criteria: * - Neither the current room nor the moving
   * object are  * *  * dark. * - The object can hear and is not a dark
   * wizard.
   */

  if (!quiet)
    if ((!is_dark(evaluation->world->database, thing) &&
         !is_dark(evaluation->world->database, loc)) ||
        (canhear && !(is_wizard(evaluation->world->database, thing) &&
                      is_dark(evaluation->world->database, thing)))) {
      notify_except2(
          evaluation, loc, thing, thing, cause,
          tprintf("%s has left.",
                  game_object_name(evaluation->world->database, thing)));
    }
}

/*
 * ---------------------------------------------------------------------------
 * * process_enter_loc: Generate messages and actions resulting from entering
 * * a place.
 */
static void process_enter_loc(EvaluationContext *evaluation, DbRef thing,
                              DbRef src, DbRef cause, int canhear, int hush) {
  DbRef loc;
  int quiet;

  loc = game_object_location(evaluation->world->database, thing);
  if ((loc == NOTHING) || (loc == src))
    return;

  /*
   * Run the ENTER attributes in the current room if we meet any of * *
   *
   * *  * * following criteria: * - The current room has wizard privs.
   * * - * * * Neither the current room nor the moving object are dark.
   * * - The * *  * moving object can hear and does not hav wizard
   * privs. * EXCEPT  * if * * we were called with the HUSH_ENTER key.
   */

  quiet = (!(is_wizard(evaluation->world->database, loc) ||
             (!is_dark(evaluation->world->database, thing) &&
              !is_dark(evaluation->world->database, loc)) ||
             (canhear && !(is_wizard(evaluation->world->database, thing) &&
                           is_dark(evaluation->world->database, thing))))) ||
          (hush & HUSH_ENTER);
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_ENTER,
                                .operation = LUA_MESSAGE_OPERATION_MOVE,
                                .object = loc,
                                .enactor = thing,
                                .cause = cause,
                                .source = src,
                                .destination = loc,
                                .silent = quiet},
                    .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_ENTER});

  /*
   * Do OXLEAVE for sending room
   */

  if ((src != NOTHING) && !quiet)
    notify_action(evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_LEAVE_DESTINATION,
                                  .operation = LUA_MESSAGE_OPERATION_MOVE,
                                  .object = src,
                                  .enactor = thing,
                                  .cause = cause,
                                  .source = src,
                                  .destination = loc}});

  /*
   * Display the 'has arrived' message if we meet all of the following
   * * * * * criteria: * - The moving object can hear. * - The object
   * is * * not * a dark wizard.
   */

  if (!quiet && canhear &&
      !(is_dark(evaluation->world->database, thing) &&
        is_wizard(evaluation->world->database, thing))) {
    notify_except2(
        evaluation, loc, thing, thing, cause,
        tprintf("%s has arrived.",
                game_object_name(evaluation->world->database, thing)));
  }
}

/*
 * ---------------------------------------------------------------------------
 * * move_object: Physically move an object from one place to another.
 * * Does not generate any messages or actions.
 */

void move_object(EvaluationContext *evaluation, DbRef thing, DbRef dest) {
  DbRef src;

  /*
   * Remove from the source location
   */

  src = game_object_location(evaluation->world->database, thing);
  if (src != NOTHING)
    game_object_set_contents(
        evaluation->world->database, src,
        remove_first(evaluation->world->database,
                     game_object_contents(evaluation->world->database, src),
                     thing));

  /*
   * Special check for HOME
   */

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);

  /*
   * Add to destination location
   */

  if (dest != NOTHING)
    game_object_set_contents(
        evaluation->world->database, dest,
        insert_first(evaluation->world->database,
                     game_object_contents(evaluation->world->database, dest),
                     thing));
  else
    game_object_set_next(evaluation->world->database, thing, NOTHING);
  game_object_set_location(evaluation->world->database, thing, dest);

  look_in(evaluation, thing, dest, LK_SHOWEXIT);
}

/*
 * ---------------------------------------------------------------------------
 * * send_dropto, process_dropped_dropto,
 * * process_sacrifice_dropto: Check for and process droptos.
 */

/*
 * send_dropto: Send an object through the dropto of a room
 */

static void send_dropto(EvaluationContext *evaluation, DbRef thing,
                        DbRef player) {
  move_via_generic(
      evaluation, thing,
      game_object_location(
          evaluation->world->database,
          game_object_location(evaluation->world->database, thing)),
      player, 0);
}

/*
 * process_dropped_dropto: Check what to do when someone drops an object.
 */

static void process_dropped_dropto(EvaluationContext *evaluation, DbRef thing,
                                   DbRef player) {
  DbRef loc;

  /* Process the dropto if the location is a room with a destination. */

  loc = game_object_location(evaluation->world->database, thing);
  if (has_dropto(evaluation->world->database, loc) &&
      (game_object_location(evaluation->world->database, loc) != NOTHING))
    send_dropto(evaluation, thing, player);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_generic: Generic move routine, generates standard messages and
 * * actions.
 */

void move_via_generic(EvaluationContext *evaluation, DbRef thing, DbRef dest,
                      DbRef cause, int hush) {
  DbRef src;
  int canhear;

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);
  src = game_object_location(evaluation->world->database, thing);
  canhear = is_hearer(evaluation, thing);
  process_leave_loc(evaluation, thing, dest, cause, canhear, hush);
  move_object(evaluation, thing, dest);
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_MOVE,
                                .operation = LUA_MESSAGE_OPERATION_MOVE,
                                .object = thing,
                                .enactor = thing,
                                .cause = cause,
                                .source = src,
                                .destination = dest},
                    .event = LUA_EVENT_MOVE});
  process_enter_loc(evaluation, thing, src, cause, canhear, hush);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_exit: Exit move routine, generic + exit messages + dropto check.
 */

void move_via_exit(EvaluationContext *evaluation, DbRef thing, DbRef dest,
                   DbRef cause, DbRef exit, int hush) {
  DbRef src;
  int canhear, darkwiz, quiet;

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);
  src = game_object_location(evaluation->world->database, thing);
  canhear = is_hearer(evaluation, thing);

  /*
   * Dark wizards don't trigger OSUCC/ASUCC
   */

  darkwiz = (is_wizard(evaluation->world->database, thing) &&
             is_dark(evaluation->world->database, thing));
  quiet = darkwiz || (hush & HUSH_EXIT);

  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_SUCCESS,
                                .operation = LUA_MESSAGE_OPERATION_TRAVERSE,
                                .object = exit,
                                .enactor = thing,
                                .cause = cause,
                                .source = src,
                                .destination = dest,
                                .silent = quiet},
                    .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_SUCCESS});
  process_leave_loc(evaluation, thing, dest, cause, canhear, hush);
  move_object(evaluation, thing, dest);

  /*
   * Dark wizards don't trigger ODROP/ADROP
   */

  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_DROP,
                                .operation = LUA_MESSAGE_OPERATION_TRAVERSE,
                                .object = exit,
                                .enactor = thing,
                                .cause = cause,
                                .source = src,
                                .destination = dest,
                                .silent = quiet},
                    .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_DROP});

  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_MOVE,
                                .operation = LUA_MESSAGE_OPERATION_MOVE,
                                .object = thing,
                                .enactor = thing,
                                .cause = cause,
                                .source = src,
                                .destination = dest},
                    .event = LUA_EVENT_MOVE});
  process_enter_loc(evaluation, thing, src, cause, canhear, hush);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_teleport: Teleport move routine, generic + teleport messages +
 * * divestiture + dropto check.
 */

int move_via_teleport(EvaluationContext *evaluation, DbRef thing, DbRef dest,
                      DbRef cause, int hush) {
  DbRef src, curr;
  int canhear, count;
  const char *failmsg;
  LuaLockInvocation lock;
  LuaLockResult result;
  const ServerConfiguration *configuration = evaluation->world->configuration;

  src = game_object_location(evaluation->world->database, thing);
  if ((dest != HOME) && is_good_obj(evaluation->world->database, src)) {
    curr = src;
    for (count = configuration->ntfy_nest_lim; count > 0; count--) {
      if (!lock_test(evaluation, thing, cause, thing, curr,
                     LUA_LOCK_TELEPORT_OUT, LUA_LOCK_OPERATION_TELEPORT_OUT,
                     false, &lock, &result)) {
        if ((thing == cause) || (cause == NOTHING))
          failmsg = "You can't teleport out!";
        else {
          failmsg = "You can't be teleported out!";
          notify_quiet(evaluation, cause, "You can't teleport that out!");
        }
        notify_lock_failure(evaluation, &lock, &result, failmsg, nullptr,
                            LUA_EVENT_TELEPORT_OUT_FAIL);
        return 0;
      }
      if (is_room(evaluation->world->database, curr))
        break;
      curr = game_object_location(evaluation->world->database, curr);
    }
  }
  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);
  canhear = is_hearer(evaluation, thing);
  if (!(hush & HUSH_LEAVE))
    notify_action(evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_TELEPORT_SOURCE,
                                  .operation = LUA_MESSAGE_OPERATION_TELEPORT,
                                  .object = thing,
                                  .enactor = thing,
                                  .cause = cause,
                                  .source = src,
                                  .destination = dest}});
  process_leave_loc(evaluation, thing, dest, NOTHING, canhear, hush);
  move_object(evaluation, thing, dest);
  if (!(hush & HUSH_ENTER))
    notify_action(evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_TELEPORT,
                                  .operation = LUA_MESSAGE_OPERATION_TELEPORT,
                                  .object = thing,
                                  .enactor = thing,
                                  .cause = cause,
                                  .source = src,
                                  .destination = dest},
                      .event = LUA_EVENT_TELEPORT});
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_MOVE,
                                .operation = LUA_MESSAGE_OPERATION_TELEPORT,
                                .object = thing,
                                .enactor = thing,
                                .cause = cause,
                                .source = src,
                                .destination = dest},
                    .event = LUA_EVENT_MOVE});
  process_enter_loc(evaluation, thing, src, NOTHING, canhear, hush);
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * move_exit: Try to move a player through an exit.
 */

void move_exit(EvaluationContext *evaluation, DbRef player, DbRef exit,
               const char *failmsg, int hush) {
  DbRef loc;
  bool silent;
  LuaLockInvocation lock;
  LuaLockResult result;

  loc = game_object_location(evaluation->world->database, exit);
  if (loc == HOME)
    loc = game_object_link(evaluation->world->database, player);
  silent = (is_wizard(evaluation->world->database, player) &&
            is_dark(evaluation->world->database, player)) ||
           (hush & HUSH_EXIT);
  lock = (LuaLockInvocation){
      .type = LUA_LOCK_DEFAULT,
      .operation = LUA_LOCK_OPERATION_TRAVERSE,
      .descriptor = evaluation->command->descriptor,
      .object = exit,
      .enactor = player,
      .cause = player,
      .subject = player,
      .silent = silent,
  };
  result = (LuaLockResult){0};
  if (is_good_obj(evaluation->world->database, loc) &&
      lock_test(evaluation, player, player, player, exit, LUA_LOCK_DEFAULT,
                LUA_LOCK_OPERATION_TRAVERSE, silent, &lock, &result)) {
    switch (typeof_obj(evaluation->world->database, loc)) {
    case TYPE_ROOM:
      move_via_exit(evaluation, player, loc, NOTHING, exit, hush);
      break;
    case TYPE_PLAYER:
    case TYPE_THING:
      if (is_going(evaluation->world->database, loc)) {
        notify(evaluation, player, "You can't go that way.");
        return;
      }
      move_via_exit(evaluation, player, loc, NOTHING, exit, hush);
      break;
    case TYPE_EXIT:
      notify(evaluation, player, "You can't go that way.");
      return;
    default:
      break;
    }
  } else {
    notify_lock_failure(evaluation, &lock, &result, failmsg, nullptr,
                        LUA_EVENT_FAIL);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_move: Move from one place to another via exits or 'home'.
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
      snprintf(buffer, MBUF_SIZE - 1, "%s goes home.",
               game_object_name(evaluation->world->database, player));
      notify_except(evaluation, loc, player, player, buffer);
    }
    /*
     * give the player the messages
     */

    for (i = 0; i < 3; i++)
      notify(evaluation, player, "There's no place like home...");
    move_via_generic(evaluation, player, HOME, NOTHING, 0);
    return;
  }
  /*
   * find the exit
   */

  init_match_check_keys(match, player, direction, TYPE_EXIT);
  match_exit(match);
  exit = match_result(match);
  switch (exit) {
  case NOTHING: /*
                 * try to force the object
                 */
    notify(evaluation, player, "You can't go that way.");
    break;
  case AMBIGUOUS:
    notify(evaluation, player, "I don't know which way you mean!");
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

void do_get(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *what = invocation->first;
  DbRef thing, playerloc, thingloc;
  const char *failmsg;
  int quiet;
  LuaLockInvocation lock;
  LuaLockResult result;

  playerloc = game_object_location(evaluation->world->database, player);
  if (!is_good_obj(evaluation->world->database, playerloc))
    return;

  /*
   * Look for the thing locally
   */

  MatchContext *match = &invocation->context->match;
  init_match_check_keys(match, player, what, TYPE_THING);
  match_neighbor(match);
  match_exit(match);
  if (is_long_fingers(evaluation->world->database, player))
    match_absolute(match); /*
                            * long fingers
                            */
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
  case TYPE_PLAYER:
  case TYPE_THING:
    /*
     * You can't take what you already have
     */

    thingloc = game_object_location(evaluation->world->database, thing);
    if (thingloc == player) {
      notify(evaluation, player, "You already have that!");
      break;
    }
    if ((key & GET_QUIET) &&
        is_controls(evaluation->world->database, player, thing))
      quiet = 1;

    if (thing == player) {
      notify(evaluation, player, "You cannot get yourself!");
    } else if (lock_test(evaluation, player, invocation->cause, player, thing,
                         LUA_LOCK_DEFAULT, LUA_LOCK_OPERATION_TAKE, quiet,
                         &lock, &result)) {
      if (thingloc !=
          game_object_location(evaluation->world->database, player)) {
        notify_printf(evaluation, thingloc, "%s was taken from you.",
                      game_object_name(evaluation->world->database, thing));
      }
      move_via_generic(evaluation, thing, player, player, 0);
      notify(evaluation, thing, "Taken.");
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
                          .silent = quiet},
              .enactor_default = "Taken.",
              .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_SUCCESS});
    } else {
      if (thingloc != game_object_location(evaluation->world->database, player))
        failmsg = "You can't take that from there.";
      else
        failmsg = "You can't pick that up.";
      notify_lock_failure(evaluation, &lock, &result, failmsg, nullptr,
                          LUA_EVENT_FAIL);
    }
    break;
  case TYPE_EXIT:
    /*
     * You can't take what you already have
     */

    thingloc = game_object_exits(evaluation->world->database, thing);
    if (thingloc == player) {
      notify(evaluation, player, "You already have that!");
      break;
    }
    /*
     * You must control either the exit or the location
     */

    playerloc = game_object_location(evaluation->world->database, player);
    if (!is_controls(evaluation->world->database, player, thing) &&
        !is_controls(evaluation->world->database, player, playerloc)) {
      notify(evaluation, player, "Permission denied.");
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
    if (!is_quiet(evaluation->world->database, player))
      notify(evaluation, player, "Exit taken.");
    break;
  default:
    notify(evaluation, player, "You can't take that!");
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
  DbRef loc, exitloc, thing;
  char *buf, *bp;
  int quiet;
  LuaLockInvocation lock;
  LuaLockResult result;

  loc = game_object_location(evaluation->world->database, player);
  if (!is_good_obj(evaluation->world->database, loc))
    return;

  MatchContext *match = &invocation->context->match;
  init_match(match, player, name, TYPE_THING);
  match_possession(match);
  match_carried_exit(match);

  switch (thing = match_result(match)) {
  case NOTHING:
    notify(evaluation, player, "You don't have that!");
    return;
  case AMBIGUOUS:
    notify(evaluation, player, "I don't know which you mean!");
    return;
  default:
    break;
  }

  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_THING:
  case TYPE_PLAYER:

    /*
     * You have to be carrying it
     */

    if ((game_object_location(evaluation->world->database, thing) != player) &&
        !is_wizard(evaluation->world->database, player)) {
      notify(evaluation, player, "You can't drop that.");
      return;
    }
    if (!lock_test(evaluation, player, invocation->cause, player, thing,
                   LUA_LOCK_DROP, LUA_LOCK_OPERATION_DROP, false, &lock,
                   &result)) {
      notify_lock_failure(evaluation, &lock, &result, "You can't drop that.",
                          nullptr, LUA_EVENT_DROP_FAIL);
      return;
    }
    /*
     * Move it
     */

    move_via_generic(evaluation, thing,
                     game_object_location(evaluation->world->database, player),
                     player, 0);
    notify(evaluation, thing, "Dropped.");
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
                                  .silent = quiet},
                      .enactor_default = "Dropped.",
                      .other_default = buf,
                      .event = quiet ? LUA_EVENT_NONE : LUA_EVENT_DROP});
    free_lbuf(buf);

    /*
     * Process droptos
     */

    process_dropped_dropto(evaluation, thing, player);

    break;
  case TYPE_EXIT:

    /*
     * You have to be carrying it
     */

    if ((game_object_exits(evaluation->world->database, thing) != player) &&
        !is_wizard(evaluation->world->database, player)) {
      notify(evaluation, player, "You can't drop that.");
      return;
    }
    if (!is_controls(evaluation->world->database, player, loc)) {
      notify(evaluation, player, "Permission denied.");
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

    if (!is_quiet(evaluation->world->database, player))
      notify(evaluation, player, "Exit dropped.");
    break;
  default:
    notify(evaluation, player, "You can't drop that.");
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_enter, do_leave: The enter and leave commands.
 */

void do_enter_internal(EvaluationContext *evaluation, DbRef player, DbRef thing,
                       int quiet) {
  DbRef loc = game_object_location(evaluation->world->database, player);
  int oattr;
  LuaLockInvocation lock;
  LuaLockResult result;

  if (player == thing) {
    notify(evaluation, player, "You can't enter yourself!");
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
  init_match(match, player, what, TYPE_THING);
  match_neighbor(match);
  if (is_long_fingers(evaluation->world->database, player))
    match_absolute(match); /*
                            * the wizard has long fingers
                            */

  if ((thing = noisy_match_result(match)) == NOTHING)
    return;

  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_PLAYER:
  case TYPE_THING:
    quiet = 0;
    if ((key & MOVE_QUIET) &&
        is_controls(evaluation->world->database, player, thing))
      quiet = 1;
    do_enter_internal(evaluation, player, thing, quiet);
    break;
  default:
    notify(evaluation, player, "Permission denied.");
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
    notify(evaluation, player, "You can't leave.");
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
