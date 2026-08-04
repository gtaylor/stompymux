/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"

#include "p.glue.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/communication/comsys.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/match.h"
#include "mux/world/object.h"
#include "mux/world/object_set.h"
#include "mux/world/walkdb.h"

extern NameTable indiv_attraccess_nametab[];

static int can_destroy_exit(EvaluationContext *evaluation, DbRef player,
                            DbRef exit) {
  DbRef loc;

  loc = game_object_exits(evaluation->world->database, exit);
  if ((loc != game_object_location(evaluation->world->database, player)) &&
      (loc != player) && !is_wizard(evaluation->world->database, player)) {
    notify_quiet(evaluation, player,
                 "You can not destroy exits in another room.");
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
    notify_quiet(evaluation, player, "Sorry, no suicide allowed.");
    return 0;
  }
  if (is_wizard(evaluation->world->database, victim)) {
    notify_quiet(evaluation, player, "You may not destroy Wizards!");
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
  if (is_safe(evaluation->world->database,
              invocation->context->world->configuration, thing, player) &&
      !(key & DEST_OVERRIDE)) {
    notify_quiet(evaluation, player,
                 "Sorry, that object is protected. Use "
                 "@destroy/override to destroy it.");
    return;
  }
  /*
   * Make sure we're not trying to destroy a special object
   */

  if (!destroyable(evaluation->world->database,
                   invocation->context->world->configuration, thing)) {
    notify_quiet(evaluation, player, "You can't destroy that!");
    return;
  }
  /*
   * Go do it
   */

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_EXIT:
    if (can_destroy_exit(evaluation, player, thing)) {
      if (is_going(evaluation->world->database, thing)) {
        notify_quiet(evaluation, player, "No sense beating a dead exit.");
      } else {
        if (is_xcode(evaluation->world->database, thing)) {
          DisposeSpecialObject(evaluation->btech, player, thing);
          c_xcode(evaluation->world->database, thing);
        }
        if (0) {
          destroy_exit(evaluation, thing);
        } else {
          notify(evaluation, player, "The exit shakes and begins to crumble.");
          s_going(evaluation->world->database, thing);
        }
      }
    }
    break;
  case OBJECT_TYPE_THING:
    if (is_going(evaluation->world->database, thing)) {
      notify_quiet(evaluation, player, "No sense beating a dead object.");
    } else {
      if (is_xcode(evaluation->world->database, thing)) {
        DisposeSpecialObject(evaluation->btech, player, thing);
        c_xcode(evaluation->world->database, thing);
      }
      if (0) {
        destroy_thing(evaluation, thing);
      } else {
        notify(evaluation, player, "The object shakes and begins to crumble.");
        s_going(evaluation->world->database, thing);
      }
    }
    break;
  case OBJECT_TYPE_PLAYER:
    if (can_destroy_player(evaluation, player, thing)) {
      if (is_going(evaluation->world->database, thing)) {
        notify_quiet(evaluation, player, "No sense beating a dead player.");
      } else {
        if (is_xcode(evaluation->world->database, thing)) {
          DisposeSpecialObject(evaluation->btech, player, thing);
          c_xcode(evaluation->world->database, thing);
        }
        if (0) {
          attribute_add_raw(evaluation->world->database, thing, A_DESTROYER,
                            tprintf("%ld", player));
          destroy_player(evaluation, thing);
        } else {
          notify(evaluation, player,
                 "The player shakes and begins to crumble.");
          s_going(evaluation->world->database, thing);
          attribute_add_raw(evaluation->world->database, thing, A_DESTROYER,
                            tprintf("%ld", player));
        }
      }
    }
    break;
  case OBJECT_TYPE_ROOM:
    if (is_going(evaluation->world->database, thing)) {
      notify_quiet(evaluation, player, "No sense beating a dead room.");
    } else {
      if (0) {
        empty_obj(evaluation, thing);
        destroy_obj(evaluation, NOTHING, thing);
      } else {
        notify_all(evaluation, thing, player,
                   "The room shakes and begins to crumble.");
        s_going(evaluation->world->database, thing);
      }
    }
  default:
    break;
  }
}
