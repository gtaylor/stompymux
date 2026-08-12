/*
 * object.c - low-level object manipulation routines
 */

#include "mux/world/database_check.h"

#include "mux/commands/command_handlers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/world/move.h"
#include "mux/world/object.h"
#include "mux/world/object_internal.h"

#define ZAP_LOC(database, i)                                                   \
  {                                                                            \
    game_object_set_location(database, i, NOTHING);                            \
    game_object_set_next(database, i, NOTHING);                                \
  }

/**
 * Look for references to OBJECT_FLAG_GOING or illegal objects.
 */
static void check_dead_refs(EvaluationContext *evaluation, bool full_check) {
  DbRef targ, i;

  DO_WHOLE_DB(evaluation->world->database, i) {
    /*
     * Check the zone
     */

    targ = game_object_zone(evaluation->world->database, i);
    if (is_good_obj(evaluation->world->database, targ)) {
      if (is_going(evaluation->world->database, targ)) {
        game_object_set_zone(evaluation->world->database, i, NOTHING);
      }
    } else if (targ != NOTHING) {
      object_log_header_error(
          evaluation, i, game_object_location(evaluation->world->database, i),
          targ, 1, "Zone", "is invalid. Cleared.");
      game_object_set_zone(evaluation->world->database, i, NOTHING);
    }
    switch (typeof_obj(evaluation->world->database, i)) {
    case OBJECT_TYPE_PLAYER:
    case OBJECT_TYPE_THING:

      if (is_going(evaluation->world->database, i))
        break;

      /*
       * Check the home
       */

      targ = game_object_link(evaluation->world->database, i);
      if (is_good_obj(evaluation->world->database, targ)) {
        if (is_going(evaluation->world->database, targ)) {
          game_object_set_link(evaluation->world->database, i,
                               new_home(evaluation, i));
        }
      } else if (targ != NOTHING) {
        object_log_header_error(
            evaluation, i, game_object_location(evaluation->world->database, i),
            targ, 1, "Home", "is invalid.  Cleared.");
        game_object_set_link(evaluation->world->database, i,
                             new_home(evaluation, i));
      }
      /*
       * Check the location
       */

      targ = game_object_location(evaluation->world->database, i);
      if (!is_good_obj(evaluation->world->database, targ)) {
        object_log_pointer_error(
            &(ObjectPointerError){.evaluation = evaluation,
                                  .prior = NOTHING,
                                  .object = i,
                                  .location = NOTHING,
                                  .reference = targ,
                                  .reference_type = "Location",
                                  .error_type = "is invalid.  Moved to home."});
        ZAP_LOC(evaluation->world->database, i);
        move_object(evaluation, i, HOME);
      }
      /*
       * Check for self-referential
       * game_object_next(evaluation->world->database, )
       */

      if (game_object_next(evaluation->world->database, i) == i) {
        object_log_simple_error(evaluation, i, NOTHING,
                                "Next points to self.  Next cleared.");
        game_object_set_next(evaluation->world->database, i, NOTHING);
      }
      break;
    case OBJECT_TYPE_ROOM:

      /*
       * Check the dropto
       */

      targ = game_object_location(evaluation->world->database, i);
      if (is_good_obj(evaluation->world->database, targ)) {
        if (is_going(evaluation->world->database, targ)) {
          game_object_set_location(evaluation->world->database, i, NOTHING);
        }
      } else if ((targ != NOTHING) && (targ != HOME)) {
        object_log_header_error(evaluation, i, NOTHING, targ, 1, "Dropto",
                                "is invalid.  Cleared.");
        game_object_set_location(evaluation->world->database, i, NOTHING);
      }
      if (full_check) {

        /*
         * NEXT should be null
         */

        if (game_object_next(evaluation->world->database, i) != NOTHING) {
          object_log_header_error(
              evaluation, i, NOTHING,
              game_object_next(evaluation->world->database, i), 1,
              "Next pointer", "should be NOTHING.  Reset.");
          game_object_set_next(evaluation->world->database, i, NOTHING);
        }
        /*
         * LINK should be null
         */

        if (game_object_link(evaluation->world->database, i) != NOTHING) {
          object_log_header_error(
              evaluation, i, NOTHING,
              game_object_link(evaluation->world->database, i), 1,
              "Link pointer ", "should be NOTHING.  Reset.");
          game_object_set_link(evaluation->world->database, i, NOTHING);
        }
      }
      break;
    case OBJECT_TYPE_EXIT:

      /*
       * If it points to something OBJECT_FLAG_GOING, set it going
       */

      targ = game_object_location(evaluation->world->database, i);
      if (is_good_obj(evaluation->world->database, targ)) {
        if (is_going(evaluation->world->database, targ)) {
          s_going(evaluation->world->database, i);
        }
      } else if (targ == HOME) {
        /*
         * null case, HOME is always valid
         */
      } else if (targ != NOTHING) {
        object_log_header_error(
            evaluation, i, game_object_exits(evaluation->world->database, i),
            targ, 1, "Destination", "is invalid.  Exit destroyed.");
        s_going(evaluation->world->database, i);
      } else {
        if (!has_contents(evaluation->world->database, targ)) {
          object_log_header_error(
              evaluation, i, game_object_exits(evaluation->world->database, i),
              targ, 1, "Destination", "is not a valid type.  Exit destroyed.");
          s_going(evaluation->world->database, i);
        }
      }

      /*
       * Check for self-referential
       * game_object_next(evaluation->world->database, )
       */

      if (game_object_next(evaluation->world->database, i) == i) {
        object_log_simple_error(evaluation, i, NOTHING,
                                "Next points to self.  Next cleared.");
        game_object_set_next(evaluation->world->database, i, NOTHING);
      }
      if (full_check) {

        /*
         * CONTENTS should be null
         */

        if (game_object_contents(evaluation->world->database, i) != NOTHING) {
          object_log_header_error(
              evaluation, i, game_object_exits(evaluation->world->database, i),
              game_object_contents(evaluation->world->database, i), 1,
              "Contents", "should be NOTHING.  Reset.");
          game_object_set_contents(evaluation->world->database, i, NOTHING);
        }
        /*
         * LINK should be null
         */

        if (game_object_link(evaluation->world->database, i) != NOTHING) {
          object_log_header_error(
              evaluation, i, game_object_exits(evaluation->world->database, i),
              game_object_link(evaluation->world->database, i), 1, "Link",
              "should be NOTHING.  Reset.");
          game_object_set_link(evaluation->world->database, i, NOTHING);
        }
      }
      break;
    case OBJECT_TYPE_GARBAGE:
      break;
    default:

      /*
       * Funny object type, destroy it
       */

      object_log_simple_error(evaluation, i, NOTHING,
                              "Funny object type.  Destroyed.");
      destroy_obj(&(ObjectDestructionRequest){
          .evaluation = evaluation, .player = NOTHING, .object = i});
    }

    if (full_check) {

      /*
       * Check for wizards
       */

      if (is_wizard(evaluation->world->database, i)) {
        if (is_player(evaluation->world->database, i)) {
          object_log_simple_error(evaluation, i, NOTHING,
                                  "Player is a WIZARD.");
        }
      }
    }
  }
}

/**
 * check_loc_exits, check_exit_chains: Validate the exits chains
 * of objects and attempt to correct problems. The following errors are
 * found and corrected:
 *       Location not in database                        - skip it.
 *       Location OBJECT_FLAG_GOING                                  - skip it.
 *       Location not a PLAYER, ROOM, or THING           - skip it.
 *       Location already visited                        - skip it.
 *       Exit/next pointer not in database               - NULL it.
 *       Member is not an EXIT                           - terminate chain.
 *       Member is OBJECT_FLAG_GOING                                 - destroy
 * exit. Member already checked (is in another list)     - terminate chain.
 *       Member in another chain (recursive check)       - terminate chain.
 *       Location of member is not specified location    - reset it.
 */
static void check_loc_exits(EvaluationContext *evaluation, DbRef loc,
                            bool full_check) {
  DbRef exit, back, temp, exitloc, dest;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  /*
   * Only check players, rooms, and things that aren't OBJECT_FLAG_GOING
   */

  if (is_exit(evaluation->world->database, loc) ||
      is_going(evaluation->world->database, loc))
    return;

  /*
   * If marked, we've checked here already
   */

  if (is_marked(evaluation->world->database, loc))
    return;
  mark(evaluation->world->database, loc);

  /*
   * Check all the exits
   */

  back = NOTHING;
  exit = game_object_exits(evaluation->world->database, loc);
  while (exit != NOTHING) {

    exitloc = NOTHING;
    dest = NOTHING;

    if (is_good_obj(evaluation->world->database, exit)) {
      exitloc = game_object_exits(evaluation->world->database, exit);
      dest = game_object_location(evaluation->world->database, exit);
    }
    if (!is_good_obj(evaluation->world->database, exit)) {

      /*
       * A bad pointer - terminate chain
       */

      object_log_pointer_error(
          &(ObjectPointerError){.evaluation = evaluation,
                                .prior = back,
                                .object = loc,
                                .location = NOTHING,
                                .reference = exit,
                                .reference_type = "Exit list",
                                .error_type = "is invalid.  List nulled."});
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, NOTHING);
      } else {
        game_object_set_exits(evaluation->world->database, loc, NOTHING);
      }
      exit = NOTHING;
    } else if (!is_exit(evaluation->world->database, exit)) {

      /*
       * Not an exit - terminate chain
       */

      object_log_pointer_error(&(ObjectPointerError){
          .evaluation = evaluation,
          .prior = back,
          .object = loc,
          .location = NOTHING,
          .reference = exit,
          .reference_type = "Exitlist member",
          .error_type = "is not an exit.  List terminated."});
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, NOTHING);
      } else {
        game_object_set_exits(evaluation->world->database, loc, NOTHING);
      }
      exit = NOTHING;
    } else if (is_going(evaluation->world->database, exit)) {

      /*
       * Going - silently filter out
       */

      temp = game_object_next(evaluation->world->database, exit);
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, temp);
      } else {
        game_object_set_exits(evaluation->world->database, loc, temp);
      }
      destroy_obj(&(ObjectDestructionRequest){
          .evaluation = evaluation, .player = NOTHING, .object = exit});
      exit = temp;
      continue;
    } else if (is_marked(evaluation->world->database, exit)) {

      /*
       * Already in another list - terminate chain
       */

      object_log_pointer_error(&(ObjectPointerError){
          .evaluation = evaluation,
          .prior = back,
          .object = loc,
          .location = NOTHING,
          .reference = exit,
          .reference_type = "Exitlist member",
          .error_type = "is in another exitlist.  Cleared."});
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, NOTHING);
      } else {
        game_object_set_exits(evaluation->world->database, loc, NOTHING);
      }
      exit = NOTHING;
    } else if (!is_good_obj(evaluation->world->database, dest) &&
               (dest != HOME) && (dest != NOTHING)) {

      /*
       * Destination is not in the db.  Null it.
       */

      object_log_pointer_error(
          &(ObjectPointerError){.evaluation = evaluation,
                                .prior = back,
                                .object = loc,
                                .location = NOTHING,
                                .reference = exit,
                                .reference_type = "Destination",
                                .error_type = "is invalid.  Cleared."});
      game_object_set_location(evaluation->world->database, exit, NOTHING);

    } else if (exitloc != loc) {

      /*
       * Exit thinks it's in another place. Check the
       * exitlist there and see if it contains this
       * exit. If it does, then our exitlist
       * somehow pointed into the middle of their
       * exitlist. If not, assume we own the exit.
       */

      check_loc_exits(evaluation, exitloc, full_check);
      if (is_marked(evaluation->world->database, exit)) {

        /*
         * It's in the other list, give it up
         */

        object_log_pointer_error(&(ObjectPointerError){
            .evaluation = evaluation,
            .prior = back,
            .object = loc,
            .location = NOTHING,
            .reference = exit,
            .reference_type = "",
            .error_type = "is in another exitlist.  List terminated."});
        if (back != NOTHING) {
          game_object_set_next(evaluation->world->database, back, NOTHING);
        } else {
          game_object_set_exits(evaluation->world->database, loc, NOTHING);
        }
        exit = NOTHING;
      } else {

        /*
         * Not in the other list, assume in ours
         */

        object_log_header_error(evaluation, exit, loc, exitloc, 1,
                                "Not on chain for location", "Reset.");
        game_object_set_exits(evaluation->world->database, exit, loc);
      }
    }
    if (exit != NOTHING) {

      /*
       * All OK (or all was made OK)
       */

      mark(evaluation->world->database, exit);
      back = exit;
      exit = game_object_next(evaluation->world->database, exit);
    }
  }
}

static void check_exit_chains(EvaluationContext *evaluation, bool full_check) {
  DbRef i;

  unmark_all(evaluation->world->database);
  DO_WHOLE_DB(evaluation->world->database, i)
  check_loc_exits(evaluation, i, full_check);
  DO_WHOLE_DB(evaluation->world->database, i) {
    if (is_exit(evaluation->world->database, i) &&
        !is_marked(evaluation->world->database, i)) {
      object_log_simple_error(evaluation, i, NOTHING,
                              "Disconnected exit.  Destroyed.");
      destroy_obj(&(ObjectDestructionRequest){
          .evaluation = evaluation, .player = NOTHING, .object = i});
    }
  }
}

/**
 * check_misplaced_obj, check_loc_contents, check_contents_chains: Validate
 * the contents chains of objects and attempt to correct problems.  The
 * following errors are found and corrected:
 *       Location not in database                        - skip it.
 *       Location OBJECT_FLAG_GOING                                  - skip it.
 *       Location not a PLAYER, ROOM, or THING           - skip it.
 *       Location already visited                        - skip it.
 *       Contents/next pointer not in database           - NULL it.
 *       Member is not an PLAYER or THING                - terminate chain.
 *       Member is OBJECT_FLAG_GOING                                 - destroy
 * exit. Member already checked (is in another list)     - terminate chain.
 *       Member in another chain (recursive check)       - terminate chain.
 *       Location of member is not specified location    - reset it.
 */

static void check_loc_contents(EvaluationContext *evaluation, DbRef loc,
                               bool full_check);

static void check_misplaced_obj(EvaluationContext *evaluation, DbRef *obj,
                                DbRef back, DbRef loc, bool full_check) {
  /*
   * Object thinks it's in another place. Check the contents list
   * there and see if it contains this object. If it does, then
   * our contents list somehow pointed into the middle of their
   * contents list and we should truncate our list. If not,
   * assume we own the object.
   */

  if (!is_good_obj(evaluation->world->database, *obj))
    return;
  loc = game_object_location(evaluation->world->database, *obj);
  unmark(evaluation->world->database, *obj);
  if (is_good_obj(evaluation->world->database, loc)) {
    check_loc_contents(evaluation, loc, full_check);
  }
  if (is_marked(evaluation->world->database, *obj)) {

    /*
     * It's in the other list, give it up
     */

    object_log_pointer_error(&(ObjectPointerError){
        .evaluation = evaluation,
        .prior = back,
        .object = loc,
        .location = NOTHING,
        .reference = *obj,
        .reference_type = "",
        .error_type = "is in another contents list.  Cleared."});
    if (back != NOTHING) {
      game_object_set_next(evaluation->world->database, back, NOTHING);
    } else {
      game_object_set_contents(evaluation->world->database, loc, NOTHING);
    }
    *obj = NOTHING;
  } else {
    /*
     * Not in the other list, assume in ours
     */

    object_log_header_error(
        evaluation, *obj, loc,
        game_object_contents(evaluation->world->database, *obj), 1, "Location",
        "is invalid.  Reset.");
    game_object_set_contents(evaluation->world->database, *obj, loc);
  }
}

static void check_loc_contents(EvaluationContext *evaluation, DbRef loc,
                               bool full_check) {
  DbRef obj, back, temp;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  /*
   * Only check players, rooms, and things that aren't OBJECT_FLAG_GOING
   */

  if (is_exit(evaluation->world->database, loc) ||
      is_going(evaluation->world->database, loc))
    return;

  /*
   * Check all the exits
   */

  back = NOTHING;
  obj = game_object_contents(evaluation->world->database, loc);
  while (obj != NOTHING) {
    if (!is_good_obj(evaluation->world->database, obj)) {

      /*
       * A bad pointer - terminate chain
       */

      object_log_pointer_error(
          &(ObjectPointerError){.evaluation = evaluation,
                                .prior = back,
                                .object = loc,
                                .location = NOTHING,
                                .reference = obj,
                                .reference_type = "Contents list",
                                .error_type = "is invalid.  Cleared."});
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, NOTHING);
      } else {
        game_object_set_contents(evaluation->world->database, loc, NOTHING);
      }
      obj = NOTHING;
    } else if (!has_location(evaluation->world->database, obj)) {

      /*
       * Not a player or thing - terminate chain
       */

      object_log_pointer_error(&(ObjectPointerError){
          .evaluation = evaluation,
          .prior = back,
          .object = loc,
          .location = NOTHING,
          .reference = obj,
          .reference_type = "",
          .error_type = "is not a player or thing.  Cleared."});
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, NOTHING);
      } else {
        game_object_set_contents(evaluation->world->database, loc, NOTHING);
      }
      obj = NOTHING;
    } else if (is_going(evaluation->world->database, obj) &&
               (typeof_obj(evaluation->world->database, obj) ==
                OBJECT_TYPE_GARBAGE)) {

      /*
       * Going - silently filter out
       */

      temp = game_object_next(evaluation->world->database, obj);
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, temp);
      } else {
        game_object_set_contents(evaluation->world->database, loc, temp);
      }
      destroy_obj(&(ObjectDestructionRequest){
          .evaluation = evaluation, .player = NOTHING, .object = obj});
      obj = temp;
      continue;
    } else if (is_marked(evaluation->world->database, obj)) {

      /*
       * Already visited - either truncate or ignore
       */

      if (game_object_location(evaluation->world->database, obj) != loc) {

        /*
         * Location wrong - either truncate or fix
         */

        check_misplaced_obj(evaluation, &obj, back, loc, full_check);
      } else {

        /*
         * Location right - recursive contents
         */
      }
    } else if (game_object_location(evaluation->world->database, obj) != loc) {

      /*
       * Location wrong - either truncate or fix
       */

      check_misplaced_obj(evaluation, &obj, back, loc, full_check);
    }
    if (obj != NOTHING) {

      /*
       * All OK (or all was made OK)
       */

      if (full_check) {

        /*
         * Check for nonwizard objects inside wizard
         * * * * * objects.
         */

        if (is_wizard(evaluation->world->database, loc) &&
            !is_wizard(evaluation->world->database, obj)) {
          object_log_simple_error(evaluation, obj, loc,
                                  "Nonwizard object inside wizard.");
        }
      }
      mark(evaluation->world->database, obj);
      back = obj;
      obj = game_object_next(evaluation->world->database, obj);
    }
  }
}

static void check_contents_chains(EvaluationContext *evaluation,
                                  bool full_check) {
  DbRef i;

  unmark_all(evaluation->world->database);
  DO_WHOLE_DB(evaluation->world->database, i)
  check_loc_contents(evaluation, i, full_check);
  DO_WHOLE_DB(evaluation->world->database, i)
  if (!is_going(evaluation->world->database, i) &&
      !is_marked(evaluation->world->database, i) &&
      has_location(evaluation->world->database, i)) {
    object_log_simple_error(
        evaluation, i, game_object_location(evaluation->world->database, i),
        "Orphaned object, moved home.");
    ZAP_LOC(evaluation->world->database, i);
    move_via_generic(&(ObjectMovementRequest){.evaluation = evaluation,
                                              .object = i,
                                              .destination = HOME,
                                              .cause = NOTHING});
  }
}

/**
 * mark_place, check_floating: Look for floating rooms not set FLOATING.
 */
static void mark_place(GameDatabase *database, DbRef loc) {
  DbRef exit;

  /*
   * If already marked, exit.  Otherwise set marked.
   */

  if (!is_good_obj(database, loc))
    return;
  if (is_marked(database, loc))
    return;
  mark(database, loc);

  /*
   * Visit all places you can get to via exits from here.
   */

  for (exit = game_object_exits(database, loc); exit != NOTHING;
       exit = game_object_next(database, exit)) {
    if (is_good_obj(database, game_object_location(database, exit)))
      mark_place(database, game_object_location(database, exit));
  }
}

static void check_floating(EvaluationContext *evaluation) {
  DbRef i;

  /*
   * Mark everyplace you can get to via exits from the starting room
   */

  unmark_all(evaluation->world->database);
  mark_place(evaluation->world->database,
             evaluation->world->configuration->start_room);

  /*
   * Look for rooms not marked and not set FLOATING
   */

  DO_WHOLE_DB(evaluation->world->database, i) {
    if (is_room(evaluation->world->database, i) &&
        !is_floating(evaluation->world->database, i) &&
        !is_going(evaluation->world->database, i) &&
        !is_marked(evaluation->world->database, i)) {
      object_log_simple_error(evaluation, i, NOTHING, "Floating room.");
    }
  }
}

/**
 * Perform a database consistency check and clean up damage.
 */
void database_check(const DatabaseCheckRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->player;
  const bool FULL_CHECK = (request->options & DBCK_FULL) != 0;

  object_make_freelist(evaluation->world->database);
  check_dead_refs(evaluation, FULL_CHECK);
  check_exit_chains(evaluation, FULL_CHECK);
  check_contents_chains(evaluation, FULL_CHECK);
  check_floating(evaluation);
  object_purge_going(evaluation, FULL_CHECK);

  if (player != NOTHING)
    notify_checked(evaluation, player, player, "Done.",
                   MSG_ME_ALL | MSG_F_DOWN);
}

void do_dbck(CommandInvocation *invocation) {
  database_check(
      &(DatabaseCheckRequest){.evaluation = &invocation->context->evaluation,
                              .player = invocation->player,
                              .options = invocation->key});
}
