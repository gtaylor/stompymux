/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_keys.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/name_table.h"
#include "mux/world/match.h"
#include "mux/world/object_lifecycle.h"
#include "mux/world/object_set.h"

typedef struct DestroyExitCheck {
  EvaluationContext *evaluation;
  DbRef player;
  DbRef exit;
} DestroyExitCheck;

static bool can_destroy_exit(const DestroyExitCheck *check) {
  EvaluationContext *evaluation = check->evaluation;
  DbRef player = check->player;
  DbRef exit = check->exit;
  DbRef loc;

  loc = game_object_exits(evaluation->world->database, exit);
  if ((loc != game_object_location(evaluation->world->database, player)) &&
      (loc != player) && !is_wizard(evaluation->world->database, player)) {
    notify_checked(evaluation, player, player,
                   "You can not destroy exits in another room.", MSG_ME);
    return false;
  }
  return true;
}

void do_destroy(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *what = invocation->first;
  DbRef thing;

  /*
   * You can destroy anything you control
   */

  thing = match_controlled_quiet(&invocation->context->match, player, what);

  /*
   * If you own a location, you can destroy its exits
   */

  if ((thing == NOTHING) &&
      is_controls(evaluation->world->database, player,
                  game_object_location(evaluation->world->database, player))) {
    init_match(&invocation->context->match, player, what, OBJECT_TYPE_EXIT);
    match_exit(&invocation->context->match);
    thing = last_match_result(&invocation->context->match);
  }
  /*
   * Return an error if we didn't find anything to destroy
   */

  if (match_status(evaluation, player, thing) == NOTHING) {
    return;
  }
  if (is_exit(evaluation->world->database, thing) &&
      !can_destroy_exit(&(DestroyExitCheck){
          .evaluation = evaluation, .player = player, .exit = thing}))
    return;

  ObjectDestroyStatus status =
      object_destroy_schedule(&(ObjectDestroyScheduleRequest){
          .evaluation = evaluation,
          .actor = player,
          .object = thing,
          .override_safe = (key & DEST_OVERRIDE) != 0});
  if (status == OBJECT_DESTROY_SAFE) {
    notify_checked(evaluation, player, player,
                   "Sorry, that object is protected. Use "
                   "@destroy/override to destroy it.",
                   MSG_ME);
    return;
  }
  if (status == OBJECT_DESTROY_PROTECTED) {
    notify_checked(evaluation, player, player, "You can't destroy that!",
                   MSG_ME);
    return;
  }
  if (status == OBJECT_DESTROY_PLAYER_PERMISSION) {
    notify_checked(evaluation, player, player, "Sorry, no suicide allowed.",
                   MSG_ME);
    return;
  }
  if (status == OBJECT_DESTROY_WIZARD_PLAYER) {
    notify_checked(evaluation, player, player, "You may not destroy Wizards!",
                   MSG_ME);
    return;
  }
  if (status == OBJECT_DESTROY_ALREADY_GOING) {
    const char *message = "No sense beating a dead object.";

    switch (typeof_obj(evaluation->world->database, thing)) {
    case OBJECT_TYPE_EXIT:
      message = "No sense beating a dead exit.";
      break;
    case OBJECT_TYPE_PLAYER:
      message = "No sense beating a dead player.";
      break;
    case OBJECT_TYPE_ROOM:
      message = "No sense beating a dead room.";
      break;
    default:
      break;
    }
    notify_checked(evaluation, player, player, message, MSG_ME);
    return;
  }

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_EXIT:
    notify_checked(evaluation, player, player,
                   "The exit shakes and begins to crumble.",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  case OBJECT_TYPE_THING:
    notify_checked(evaluation, player, player,
                   "The object shakes and begins to crumble.",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  case OBJECT_TYPE_PLAYER:
    notify_checked(evaluation, player, player,
                   "The player shakes and begins to crumble.",
                   MSG_ME_ALL | MSG_F_DOWN);
    break;
  case OBJECT_TYPE_ROOM:
    notify_checked(evaluation, thing, player,
                   "The room shakes and begins to crumble.",
                   MSG_ME_ALL | MSG_NBR_EXITS | MSG_F_UP | MSG_F_CONTENTS);
    break;
  default:
    break;
  }
}
