/*
 * move.c -- Routines for moving about
 */

#include "mux/world/move.h"
#include "mux/commands/action_messages.h"
#include "mux/commands/look.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h" // IWYU pragma: keep
#include "mux/support/formatting.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/move_internal.h"
#include "mux/world/object_list.h"

/*
 * ---------------------------------------------------------------------------
 * * process_leave_loc: Generate messages and actions resulting from leaving
 * * a place.
 */

typedef struct LocationTransition {
  EvaluationContext *evaluation;
  DbRef object;
  DbRef other_location;
  DbRef cause;
  bool can_hear;
  int hush;
} LocationTransition;

static void process_leave_loc(const LocationTransition *transition) {
  EvaluationContext *evaluation = transition->evaluation;
  DbRef thing = transition->object;
  DbRef dest = transition->other_location;
  DbRef cause = transition->cause;
  bool canhear = transition->can_hear;
  int hush = transition->hush;
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
      notify_excluding(&(ExcludingNotification){
          .evaluation = evaluation,
          .location = loc,
          .sender = thing,
          .exceptions = {thing, cause},
          .exception_count = 2,
          .message =
              tprintf("%s has left.",
                      game_object_name(evaluation->world->database, thing))});
    }
}

/*
 * ---------------------------------------------------------------------------
 * * process_enter_loc: Generate messages and actions resulting from entering
 * * a place.
 */
static void process_enter_loc(const LocationTransition *transition) {
  EvaluationContext *evaluation = transition->evaluation;
  DbRef thing = transition->object;
  DbRef src = transition->other_location;
  DbRef cause = transition->cause;
  bool canhear = transition->can_hear;
  int hush = transition->hush;
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
    notify_excluding(&(ExcludingNotification){
        .evaluation = evaluation,
        .location = loc,
        .sender = thing,
        .exceptions = {thing, cause},
        .exception_count = 2,
        .message =
            tprintf("%s has arrived.",
                    game_object_name(evaluation->world->database, thing))});
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

  look_in(&(LookRequest){.evaluation = evaluation,
                         .viewer = thing,
                         .location = dest,
                         .key = LK_SHOWEXIT});
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
  move_via_generic(&(ObjectMovementRequest){
      .evaluation = evaluation,
      .object = thing,
      .destination = game_object_location(
          evaluation->world->database,
          game_object_location(evaluation->world->database, thing)),
      .cause = player});
}

/*
 * process_dropped_dropto: Check what to do when someone drops an object.
 */

void process_dropped_dropto(EvaluationContext *evaluation, DbRef thing,
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

void move_via_generic(const ObjectMovementRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef thing = request->object;
  DbRef dest = request->destination;
  DbRef cause = request->cause;
  int hush = request->hush;
  DbRef src;
  int canhear;

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);
  src = game_object_location(evaluation->world->database, thing);
  canhear = is_hearer(evaluation, thing);
  process_leave_loc(&(LocationTransition){.evaluation = evaluation,
                                          .object = thing,
                                          .other_location = dest,
                                          .cause = cause,
                                          .can_hear = canhear,
                                          .hush = hush});
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
  process_enter_loc(&(LocationTransition){.evaluation = evaluation,
                                          .object = thing,
                                          .other_location = src,
                                          .cause = cause,
                                          .can_hear = canhear,
                                          .hush = hush});
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_exit: Exit move routine, generic + exit messages + dropto check.
 */

void move_via_exit(const ExitMovementRequest *request) {
  EvaluationContext *evaluation = request->movement.evaluation;
  DbRef thing = request->movement.object;
  DbRef dest = request->movement.destination;
  DbRef cause = request->movement.cause;
  DbRef exit = request->exit;
  int hush = request->movement.hush;
  DbRef src;
  int canhear;
  int darkwiz;
  int quiet;

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
  process_leave_loc(&(LocationTransition){.evaluation = evaluation,
                                          .object = thing,
                                          .other_location = dest,
                                          .cause = cause,
                                          .can_hear = canhear,
                                          .hush = hush});
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
  process_enter_loc(&(LocationTransition){.evaluation = evaluation,
                                          .object = thing,
                                          .other_location = src,
                                          .cause = cause,
                                          .can_hear = canhear,
                                          .hush = hush});
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_teleport: Teleport move routine, generic + teleport messages +
 * * divestiture + dropto check.
 */

int move_via_teleport(const ObjectMovementRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef thing = request->object;
  DbRef dest = request->destination;
  DbRef cause = request->cause;
  int hush = request->hush;
  DbRef src;
  DbRef curr;
  int canhear;
  int count;
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
        if ((thing == cause) || (cause == NOTHING)) {
          failmsg = "You can't teleport out!";
        } else {
          failmsg = "You can't be teleported out!";
          notify_checked(evaluation, cause, cause,
                         "You can't teleport that out!", MSG_ME);
        }
        notify_lock_failure(
            &(LockFailureNotification){.evaluation = evaluation,
                                       .invocation = &lock,
                                       .result = &result,
                                       .enactor_default = failmsg,
                                       .event = LUA_EVENT_TELEPORT_OUT_FAIL});
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
  process_leave_loc(&(LocationTransition){.evaluation = evaluation,
                                          .object = thing,
                                          .other_location = dest,
                                          .cause = NOTHING,
                                          .can_hear = canhear,
                                          .hush = hush});
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
  process_enter_loc(&(LocationTransition){.evaluation = evaluation,
                                          .object = thing,
                                          .other_location = src,
                                          .cause = NOTHING,
                                          .can_hear = canhear,
                                          .hush = hush});
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
    case OBJECT_TYPE_ROOM:
      move_via_exit(
          &(ExitMovementRequest){.movement = {.evaluation = evaluation,
                                              .object = player,
                                              .destination = loc,
                                              .cause = NOTHING,
                                              .hush = hush},
                                 .exit = exit});
      break;
    case OBJECT_TYPE_PLAYER:
    case OBJECT_TYPE_THING:
      if (is_going(evaluation->world->database, loc)) {
        notify_checked(evaluation, player, player, "You can't go that way.",
                       MSG_ME_ALL | MSG_F_DOWN);
        return;
      }
      move_via_exit(
          &(ExitMovementRequest){.movement = {.evaluation = evaluation,
                                              .object = player,
                                              .destination = loc,
                                              .cause = NOTHING,
                                              .hush = hush},
                                 .exit = exit});
      break;
    case OBJECT_TYPE_EXIT:
      notify_checked(evaluation, player, player, "You can't go that way.",
                     MSG_ME_ALL | MSG_F_DOWN);
      return;
    default:
      break;
    }
  } else {
    notify_lock_failure(&(LockFailureNotification){.evaluation = evaluation,
                                                   .invocation = &lock,
                                                   .result = &result,
                                                   .enactor_default = failmsg,
                                                   .event = LUA_EVENT_FAIL});
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_move: Move from one place to another via exits or 'home'.
 */
