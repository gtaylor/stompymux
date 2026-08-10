/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include "btech/special_objects.h"
#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_keys.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h" // IWYU pragma: keep
#include "mux/server/server_control.h"
#include "mux/support/formatting.h"
#include "mux/support/name_table.h"
#include "mux/world/match.h"
#include "mux/world/object.h"
#include "mux/world/object_set.h"

extern NameTable indiv_attraccess_nametab[];

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
    return 0;
  }
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * destroyable: Indicates if target of a @destroy is a 'special' object in
 * * the database.
 */

static int destroyable(GameDatabase *database,
                       const ServerConfiguration *configuration, DbRef victim) {
  if ((victim == configuration->default_home) ||
      (victim == configuration->start_home) ||
      (victim == configuration->start_room) || (victim == (DbRef)0) ||
      is_god(database, victim))
    return 0;
  return 1;
}

static int can_destroy_player(EvaluationContext *evaluation, DbRef player,
                              DbRef victim) {
  if (!is_wizard(evaluation->world->database, player)) {
    notify_checked(evaluation, player, player, "Sorry, no suicide allowed.",
                   MSG_ME);
    return 0;
  }
  if (is_wizard(evaluation->world->database, victim)) {
    notify_checked(evaluation, player, player, "You may not destroy Wizards!",
                   MSG_ME);
    return 0;
  }
  return 1;
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
  if (is_safe(evaluation->world->database, thing) && !(key & DEST_OVERRIDE)) {
    notify_checked(evaluation, player, player,
                   "Sorry, that object is protected. Use "
                   "@destroy/override to destroy it.",
                   MSG_ME);
    return;
  }
  /*
   * Make sure we're not trying to destroy a special object
   */

  if (!destroyable(evaluation->world->database,
                   invocation->context->world->configuration, thing)) {
    notify_checked(evaluation, player, player, "You can't destroy that!",
                   MSG_ME);
    return;
  }
  /*
   * Go do it
   */

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_EXIT:
    if (can_destroy_exit(&(DestroyExitCheck){
            .evaluation = evaluation, .player = player, .exit = thing})) {
      if (is_going(evaluation->world->database, thing)) {
        notify_checked(evaluation, player, player,
                       "No sense beating a dead exit.", MSG_ME);
      } else {
        if (is_xcode(evaluation->world->database, thing)) {
          btech_special_object_dispose(&(BtechSpecialObjectAction){
              .context = evaluation->btech, .actor = player, .object = thing});
          c_xcode(evaluation->world->database, thing);
        }
        if (0) {
          destroy_exit(evaluation, thing);
        } else {
          notify_checked(evaluation, player, player,
                         "The exit shakes and begins to crumble.",
                         MSG_ME_ALL | MSG_F_DOWN);
          s_going(evaluation->world->database, thing);
        }
      }
    }
    break;
  case OBJECT_TYPE_THING:
    if (is_going(evaluation->world->database, thing)) {
      notify_checked(evaluation, player, player,
                     "No sense beating a dead object.", MSG_ME);
    } else {
      if (is_xcode(evaluation->world->database, thing)) {
        btech_special_object_dispose(&(BtechSpecialObjectAction){
            .context = evaluation->btech, .actor = player, .object = thing});
        c_xcode(evaluation->world->database, thing);
      }
      if (0) {
        destroy_thing(evaluation, thing);
      } else {
        notify_checked(evaluation, player, player,
                       "The object shakes and begins to crumble.",
                       MSG_ME_ALL | MSG_F_DOWN);
        s_going(evaluation->world->database, thing);
      }
    }
    break;
  case OBJECT_TYPE_PLAYER:
    if (can_destroy_player(evaluation, player, thing)) {
      if (is_going(evaluation->world->database, thing)) {
        notify_checked(evaluation, player, player,
                       "No sense beating a dead player.", MSG_ME);
      } else {
        if (is_xcode(evaluation->world->database, thing)) {
          btech_special_object_dispose(&(BtechSpecialObjectAction){
              .context = evaluation->btech, .actor = player, .object = thing});
          c_xcode(evaluation->world->database, thing);
        }
        if (0) {
          attribute_add_raw(evaluation->world->database, thing, A_DESTROYER,
                            tprintf("%ld", player));
          destroy_player(evaluation, thing);
        } else {
          notify_checked(evaluation, player, player,
                         "The player shakes and begins to crumble.",
                         MSG_ME_ALL | MSG_F_DOWN);
          s_going(evaluation->world->database, thing);
          attribute_add_raw(evaluation->world->database, thing, A_DESTROYER,
                            tprintf("%ld", player));
        }
      }
    }
    break;
  case OBJECT_TYPE_ROOM:
    if (is_going(evaluation->world->database, thing)) {
      notify_checked(evaluation, player, player,
                     "No sense beating a dead room.", MSG_ME);
    } else {
      if (0) {
        empty_obj(evaluation, thing);
        destroy_obj(&(ObjectDestructionRequest){
            .evaluation = evaluation, .player = NOTHING, .object = thing});
      } else {
        notify_checked(evaluation, thing, player,
                       "The room shakes and begins to crumble.",
                       MSG_ME_ALL | MSG_NBR_EXITS | MSG_F_UP | MSG_F_CONTENTS);
        s_going(evaluation->world->database, thing);
      }
    }
    break;
  default:
    break;
  }
}
