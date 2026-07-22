/*
 * object.c - low-level object manipulation routines
 */

#include "mux/world/object.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/world_context.h"

#define IS_CLEAN(database, i)                                                  \
  (is_flag_set((database), (i), TYPE_GARBAGE, GOING) &&                        \
   (game_object_location(database, i) == NOTHING) &&                           \
   (game_object_contents(database, i) == NOTHING) &&                           \
   (game_object_exits(database, i) == NOTHING) &&                              \
   (game_object_next(database, i) == NOTHING))

#define ZAP_LOC(database, i)                                                   \
  {                                                                            \
    game_object_set_location(database, i, NOTHING);                            \
    game_object_set_next(database, i, NOTHING);                                \
  }

/**
 * Log_pointer_err, Log_header_err, Log_simple_damage: Write errors to the
 * log file.
 */
static void Log_pointer_err(EvaluationContext *evaluation, DbRef prior,
                            DbRef obj, DbRef loc, DbRef ref,
                            const char *reftype, const char *errtype) {
  STARTLOG(evaluation->log, LOG_PROBLEMS, "OBJ", "DAMAG") {
    log_type_and_name(evaluation->log, obj);
    if (loc != NOTHING) {
      log_text(" in ");
      log_type_and_name(evaluation->log, loc);
    }
    log_text(": ");
    if (prior == NOTHING) {
      log_text(reftype);
    } else {
      log_text("Next pointer");
    }
    log_text(" ");
    log_type_and_name(evaluation->log, ref);
    log_text(" ");
    log_text(errtype);
    ENDLOG(evaluation->log);
  }
}

static void Log_header_err(EvaluationContext *evaluation, DbRef obj, DbRef loc,
                           DbRef val, int is_object, const char *valtype,
                           const char *errtype) {
  STARTLOG(evaluation->log, LOG_PROBLEMS, "OBJ", "DAMAG") {
    log_type_and_name(evaluation->log, obj);
    if (loc != NOTHING) {
      log_text(" in ");
      log_type_and_name(evaluation->log, loc);
    }
    log_text(": ");
    log_text(valtype);
    log_text(" ");
    if (is_object)
      log_type_and_name(evaluation->log, val);
    else
      log_number((int)val);
    log_text(" ");
    log_text(errtype);
    ENDLOG(evaluation->log);
  }
}

static void log_simple_error(EvaluationContext *evaluation, DbRef obj,
                             DbRef loc, const char *errtype) {
  STARTLOG(evaluation->log, LOG_PROBLEMS, "OBJ", "DAMAG") {
    log_type_and_name(evaluation->log, obj);
    if (loc != NOTHING) {
      log_text(" in ");
      log_type_and_name(evaluation->log, loc);
    }
    log_text(": ");
    log_text(errtype);
    ENDLOG(evaluation->log);
  }
}

/**
 * start_home, default_home, can_set_home, new_home, clone_home:
 * Routines for validating and determining homes.
 */
DbRef start_home(WorldContext *world) {
  if (world->configuration->start_home != NOTHING)
    return world->configuration->start_home;
  return world->configuration->start_room;
}

DbRef default_home(WorldContext *world) {
  if (world->configuration->default_home != NOTHING)
    return world->configuration->default_home;
  if (world->configuration->start_home != NOTHING)
    return world->configuration->start_home;
  return world->configuration->start_room;
}

int can_set_home(EvaluationContext *evaluation, DbRef player, DbRef thing,
                 DbRef home) {
  if (!is_good_obj(evaluation->world->database, player) ||
      !is_good_obj(evaluation->world->database, home) || (thing == home))
    return 0;

  switch (typeof_obj(evaluation->world->database, home)) {
  case TYPE_PLAYER:
  case TYPE_ROOM:
  case TYPE_THING:
    if (is_going(evaluation->world->database, home))
      return 0;
    if (is_controls(evaluation->world->database, player, home))
      return 1;
  default:
    break;
  }
  return 0;
}

DbRef new_home(EvaluationContext *evaluation, DbRef player) {
  WorldContext *world = evaluation->world;
  DbRef loc;

  loc = game_object_location(evaluation->world->database, player);
  if (can_set_home(evaluation, player, player, loc))
    return loc;
  loc = game_object_link(evaluation->world->database, player);
  if (can_set_home(evaluation, player, player, loc))
    return loc;
  return default_home(world);
}

DbRef clone_home(EvaluationContext *evaluation, DbRef player, DbRef thing) {
  DbRef loc;

  loc = game_object_link(evaluation->world->database, thing);
  if (can_set_home(evaluation, player, player, loc))
    return loc;
  return new_home(evaluation, player);
}

/**
 * Build a freelist
 */
static void make_freelist(GameDatabase *database) {
  DbRef i;

  database->freelist = NOTHING;
  DO_WHOLE_DB_REV(database, i) {
    if (IS_CLEAN(database, i)) {
      game_object_set_link(database, i, database->freelist);
      database->freelist = i;
    }
  }
}

/** Apply the configured Lua parent for a newly created object. */
void object_apply_default_lua_parent(EvaluationContext *evaluation,
                                     DbRef object, int object_type) {
  ServerConfiguration *configuration = evaluation->world->configuration;
  char *path;

  switch (object_type) {
  case TYPE_THING:
    path = configuration->default_thing_lua_parent;
    break;
  case TYPE_ROOM:
    path = configuration->default_room_lua_parent;
    break;
  case TYPE_EXIT:
    path = configuration->default_exit_lua_parent;
    break;
  case TYPE_PLAYER:
    path = configuration->default_player_lua_parent;
    break;
  default:
    return;
  }

  if (*path)
    attribute_add_raw(evaluation->world->database, object, A_LUAPARENT, path);
}

/**
 * Create an object of the indicated type.
 */
DbRef create_obj(EvaluationContext *evaluation, DbRef player, int objtype,
                 char *name) {
  DbRef obj;
  int okname = 0;
  Flag f1, f2, f3;
  time_t tt;
  char *buff;

  switch (objtype) {
  case TYPE_ROOM:
    f1 = evaluation->world->configuration->default_room_flags.word1;
    f2 = evaluation->world->configuration->default_room_flags.word2;
    f3 = evaluation->world->configuration->default_room_flags.word3;
    okname = ok_name(evaluation->world->configuration, name);
    break;
  case TYPE_THING:
    f1 = evaluation->world->configuration->default_thing_flags.word1;
    f2 = evaluation->world->configuration->default_thing_flags.word2;
    f3 = evaluation->world->configuration->default_thing_flags.word3;
    okname = ok_name(evaluation->world->configuration, name);
    break;
  case TYPE_EXIT:
    f1 = evaluation->world->configuration->default_exit_flags.word1;
    f2 = evaluation->world->configuration->default_exit_flags.word2;
    f3 = evaluation->world->configuration->default_exit_flags.word3;
    okname = ok_name(evaluation->world->configuration, name);
    break;
  case TYPE_PLAYER:
    f1 = evaluation->world->configuration->default_player_flags.word1;
    f2 = evaluation->world->configuration->default_player_flags.word2;
    f3 = evaluation->world->configuration->default_player_flags.word3;
    buff = munge_space(name);
    if (!badname_check(evaluation->world, buff)) {
      notify(evaluation, player, "That name is not allowed.");
      free_lbuf(buff);
      return NOTHING;
    }
    if (*buff) {
      okname = ok_player_name(evaluation->world->configuration, buff);
      if (!okname) {
        notify(evaluation, player, "That's a silly name for a player.");
        free_lbuf(buff);
        return NOTHING;
      }
    }
    if (okname) {
      okname = (lookup_player(evaluation->world, NOTHING, buff, 0) == NOTHING);
      if (!okname) {
        notify_printf(evaluation, player, "The name %s is already taken.",
                      name);
        free_lbuf(buff);
        return NOTHING;
      }
    }
    free_lbuf(buff);
    break;
  default:
    log_simple(evaluation->log, LOG_BUGS, "BUG", "OTYPE",
               tprintf("Bad object type in create_obj: %d.", objtype));
    return NOTHING;
  }

  if (objtype != TYPE_PLAYER &&
      !is_good_obj(evaluation->world->database, player))
    return NOTHING;

  /*
   * Get the first object from the freelist. If the object is not
   * clean, discard the remainder of the freelist and go get a
   * completely new object.
   */

  obj = NOTHING;
  if (evaluation->world->database->freelist != NOTHING) {
    obj = evaluation->world->database->freelist;
    if (is_good_obj(evaluation->world->database, obj) &&
        IS_CLEAN(evaluation->world->database, obj)) {
      evaluation->world->database->freelist =
          game_object_link(evaluation->world->database, obj);
    } else {
      log_simple(evaluation->log, LOG_PROBLEMS, "FRL", "DAMAG",
                 tprintf("Freelist damaged, bad object #%ld.", obj));
      obj = NOTHING;
      evaluation->world->database->freelist = NOTHING;
    }
  }
  if (obj == NOTHING) {
    obj = evaluation->world->database->top;
    db_grow(evaluation->world->database, evaluation->world->database->top + 1);
  }
  attribute_free(evaluation->world->database, obj); // Just in case...

  /*
   * Set things up according to the object type
   */

  game_object_set_location(evaluation->world->database, obj, NOTHING);
  game_object_set_contents(evaluation->world->database, obj, NOTHING);
  game_object_set_exits(evaluation->world->database, obj, NOTHING);
  game_object_set_next(evaluation->world->database, obj, NOTHING);
  game_object_set_link(evaluation->world->database, obj, NOTHING);

  if (objtype == TYPE_PLAYER) {
    DbRef zone = evaluation->world->configuration->player_zone > 0
                     ? evaluation->world->configuration->player_zone
                     : NOTHING;
    game_object_set_zone(evaluation->world->database, obj, zone);
  } else {
    game_object_set_zone(evaluation->world->database, obj,
                         game_object_zone(evaluation->world->database, player));
  }

  game_object_set_flags(evaluation->world->database, obj, objtype | f1);
  game_object_set_flags2(evaluation->world->database, obj, f2);
  game_object_set_flags3(evaluation->world->database, obj, f3);
  unmark(evaluation->world->database, obj);
  buff = munge_space((char *)name);
  object_name_set(evaluation->world->database, obj, buff);
  free_lbuf(buff);

  if (objtype == TYPE_PLAYER) {
    time(&tt);
    buff = (char *)ctime(&tt);
    buff[strlen(buff) - 1] = '\0';
    attribute_add_raw(evaluation->world->database, obj, A_LAST, buff);

    add_player_name(evaluation->world, obj,
                    game_object_name(evaluation->world->database, obj));
  }
  object_apply_default_lua_parent(evaluation, obj, objtype);
  make_freelist(evaluation->world->database);
  return obj;
}

/**
 * Destroy an object. Assumes it has already been removed from
 * all lists and has no contents or exits.
 */
void destroy_obj(EvaluationContext *evaluation, DbRef player, DbRef obj) {
  AttributeStack *sp, *next;

  if (!is_good_obj(evaluation->world->database, obj))
    return;

  /* Halt any pending commands. */
  halt_que(evaluation->runtime->commands, NOTHING, obj);

  if ((player != NOTHING) && !is_quiet(evaluation->world->database, player)) {
    if (!is_quiet(evaluation->world->database, obj))
      notify(evaluation, player, "Destroyed.");
  }

  attribute_free(evaluation->world->database, obj);
  /* object_name_set()'s parameter isn't const-correct; "Garbage" is only
     read (copied) here. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  object_name_set(evaluation->world->database, obj, (char *)"Garbage");
#pragma clang diagnostic pop
  game_object_set_flags(evaluation->world->database, obj,
                        (TYPE_GARBAGE | GOING));
  game_object_set_flags2(evaluation->world->database, obj, 0);
  game_object_set_flags3(evaluation->world->database, obj, 0);
  game_object_set_powers(evaluation->world->database, obj, 0);
  game_object_set_powers2(evaluation->world->database, obj, 0);
  game_object_set_location(evaluation->world->database, obj, NOTHING);
  game_object_set_contents(evaluation->world->database, obj, NOTHING);
  game_object_set_exits(evaluation->world->database, obj, NOTHING);
  game_object_set_next(evaluation->world->database, obj, NOTHING);
  game_object_set_link(evaluation->world->database, obj, NOTHING);
  game_object_set_zone(evaluation->world->database, obj, NOTHING);

  /*
   * Clear the stack
   */
  for (sp = game_object_stack(evaluation->world->database, obj); sp != nullptr;
       sp = next) {
    next = sp->next;
    free_lbuf(sp->data);
    free(sp);
  }

  game_object_set_stack(evaluation->world->database, obj, nullptr);

  if (evaluation->world->configuration->have_comsys)
    toast_player(evaluation, obj);

  make_freelist(evaluation->world->database);
  return;
}

/**
 * Empties the contents of a GOING object.
 */
void empty_obj(EvaluationContext *evaluation, DbRef obj) {
  DbRef targ, next;

  /*
   * Send the contents home
   */

  SAFE_DOLIST(evaluation->world->database, targ, next,
              game_object_contents(evaluation->world->database, obj)) {
    if (!has_location(evaluation->world->database, targ)) {
      log_simple_error(evaluation, targ, obj,
                       "Funny object type in contents list of GOING location. "
                       "Flush terminated.");
      break;
    } else if (game_object_location(evaluation->world->database, targ) != obj) {
      Log_header_err(evaluation, targ, obj,
                     game_object_location(evaluation->world->database, targ), 1,
                     "Location",
                     "indicates object really in another location during "
                     "cleanup of GOING location.  Flush terminated.");
      break;
    } else {
      ZAP_LOC(evaluation->world->database, targ);
      if (game_object_link(evaluation->world->database, targ) == obj) {
        game_object_set_link(evaluation->world->database, targ,
                             new_home(evaluation, targ));
      }
      move_via_generic(evaluation, targ, HOME, NOTHING, 0);
    }
  }

  /*
   * Destroy the exits
   */

  SAFE_DOLIST(evaluation->world->database, targ, next,
              game_object_exits(evaluation->world->database, obj)) {
    if (!is_exit(evaluation->world->database, targ)) {
      log_simple_error(
          evaluation, targ, obj,
          "Funny object type in exit list of GOING location. Flush "
          "terminated.");
      break;
    } else if (game_object_exits(evaluation->world->database, targ) != obj) {
      Log_header_err(evaluation, targ, obj,
                     game_object_exits(evaluation->world->database, targ), 1,
                     "Location",
                     "indicates exit really in another location during cleanup "
                     "of GOING location.  Flush terminated.");
      break;
    } else {
      destroy_obj(evaluation, NOTHING, targ);
    }
  }
}

/**
 * Destroys an exit.
 */
void destroy_exit(EvaluationContext *evaluation, DbRef exit) {
  DbRef loc;

  loc = game_object_exits(evaluation->world->database, exit);
  game_object_set_exits(
      evaluation->world->database, loc,
      remove_first(evaluation->world->database,
                   game_object_exits(evaluation->world->database, loc), exit));
  destroy_obj(evaluation, NOTHING, exit);
}

/**
 * Destroys a thing.
 */
void destroy_thing(EvaluationContext *evaluation, DbRef thing) {
  move_via_generic(evaluation, thing, NOTHING, NOTHING, 0);
  empty_obj(evaluation, thing);
  destroy_obj(evaluation, NOTHING, thing);
}

/**
 * Destroys a player.
 */
void destroy_player(EvaluationContext *evaluation, DbRef victim) {
  DbRef player;
  long aflags;
  char *buf;

  /*
   * Bye bye...
   */
  player = (DbRef)atoi(
      attribute_get_raw(evaluation->world->database, victim, A_DESTROYER));
  toast_player(evaluation, victim);
  boot_off(evaluation->world->descriptors, victim, "You have been destroyed!");
  halt_que(evaluation->runtime->commands, victim, NOTHING);

  /*
   * Remove the name from the name hash table
   */

  delete_player_name(evaluation->world, victim,
                     game_object_name(evaluation->world->database, victim));
  buf = attribute_get(evaluation->world->database, victim, A_ALIAS, &aflags);
  delete_player_name(evaluation->world, victim, buf);
  free_lbuf(buf);

  move_via_generic(evaluation, victim, NOTHING, player, 0);
  destroy_obj(evaluation, NOTHING, victim);
}

/**
 * Purges a GOING object.
 */
static void purge_going(EvaluationContext *evaluation, bool full_check) {
  DbRef i;

  DO_WHOLE_DB(evaluation->world->database, i) {
    if (!is_going(evaluation->world->database, i))
      continue;

    switch (typeof_obj(evaluation->world->database, i)) {
    case TYPE_PLAYER:
      destroy_player(evaluation, i);
      break;
    case TYPE_ROOM:

      /*
       * Room scheduled for destruction... do it
       */

      empty_obj(evaluation, i);
      destroy_obj(evaluation, NOTHING, i);
      break;
    case TYPE_THING:
      destroy_thing(evaluation, i);
      break;
    case TYPE_EXIT:
      destroy_exit(evaluation, i);
      break;
    case TYPE_GARBAGE:
      break;
    default:

      /*
       * Something else... How did this happen?
       */

      log_simple_error(evaluation, i, NOTHING,
                       "GOING object with unexpected type.  Destroyed.");
      destroy_obj(evaluation, NOTHING, i);
    }
  }
}

/**
 * Look for references to GOING or illegal objects.
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
      Log_header_err(evaluation, i,
                     game_object_location(evaluation->world->database, i), targ,
                     1, "Zone", "is invalid. Cleared.");
      game_object_set_zone(evaluation->world->database, i, NOTHING);
    }
    switch (typeof_obj(evaluation->world->database, i)) {
    case TYPE_PLAYER:
    case TYPE_THING:

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
        Log_header_err(evaluation, i,
                       game_object_location(evaluation->world->database, i),
                       targ, 1, "Home", "is invalid.  Cleared.");
        game_object_set_link(evaluation->world->database, i,
                             new_home(evaluation, i));
      }
      /*
       * Check the location
       */

      targ = game_object_location(evaluation->world->database, i);
      if (!is_good_obj(evaluation->world->database, targ)) {
        Log_pointer_err(evaluation, NOTHING, i, NOTHING, targ, "Location",
                        "is invalid.  Moved to home.");
        ZAP_LOC(evaluation->world->database, i);
        move_object(evaluation, i, HOME);
      }
      /*
       * Check for self-referential
       * game_object_next(evaluation->world->database, )
       */

      if (game_object_next(evaluation->world->database, i) == i) {
        log_simple_error(evaluation, i, NOTHING,
                         "Next points to self.  Next cleared.");
        game_object_set_next(evaluation->world->database, i, NOTHING);
      }
      break;
    case TYPE_ROOM:

      /*
       * Check the dropto
       */

      targ = game_object_location(evaluation->world->database, i);
      if (is_good_obj(evaluation->world->database, targ)) {
        if (is_going(evaluation->world->database, targ)) {
          game_object_set_location(evaluation->world->database, i, NOTHING);
        }
      } else if ((targ != NOTHING) && (targ != HOME)) {
        Log_header_err(evaluation, i, NOTHING, targ, 1, "Dropto",
                       "is invalid.  Cleared.");
        game_object_set_location(evaluation->world->database, i, NOTHING);
      }
      if (full_check) {

        /*
         * NEXT should be null
         */

        if (game_object_next(evaluation->world->database, i) != NOTHING) {
          Log_header_err(evaluation, i, NOTHING,
                         game_object_next(evaluation->world->database, i), 1,
                         "Next pointer", "should be NOTHING.  Reset.");
          game_object_set_next(evaluation->world->database, i, NOTHING);
        }
        /*
         * LINK should be null
         */

        if (game_object_link(evaluation->world->database, i) != NOTHING) {
          Log_header_err(evaluation, i, NOTHING,
                         game_object_link(evaluation->world->database, i), 1,
                         "Link pointer ", "should be NOTHING.  Reset.");
          game_object_set_link(evaluation->world->database, i, NOTHING);
        }
      }
      break;
    case TYPE_EXIT:

      /*
       * If it points to something GOING, set it going
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
        Log_header_err(evaluation, i,
                       game_object_exits(evaluation->world->database, i), targ,
                       1, "Destination", "is invalid.  Exit destroyed.");
        s_going(evaluation->world->database, i);
      } else {
        if (!has_contents(evaluation->world->database, targ)) {
          Log_header_err(
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
        log_simple_error(evaluation, i, NOTHING,
                         "Next points to self.  Next cleared.");
        game_object_set_next(evaluation->world->database, i, NOTHING);
      }
      if (full_check) {

        /*
         * CONTENTS should be null
         */

        if (game_object_contents(evaluation->world->database, i) != NOTHING) {
          Log_header_err(evaluation, i,
                         game_object_exits(evaluation->world->database, i),
                         game_object_contents(evaluation->world->database, i),
                         1, "Contents", "should be NOTHING.  Reset.");
          game_object_set_contents(evaluation->world->database, i, NOTHING);
        }
        /*
         * LINK should be null
         */

        if (game_object_link(evaluation->world->database, i) != NOTHING) {
          Log_header_err(evaluation, i,
                         game_object_exits(evaluation->world->database, i),
                         game_object_link(evaluation->world->database, i), 1,
                         "Link", "should be NOTHING.  Reset.");
          game_object_set_link(evaluation->world->database, i, NOTHING);
        }
      }
      break;
    case TYPE_GARBAGE:
      break;
    default:

      /*
       * Funny object type, destroy it
       */

      log_simple_error(evaluation, i, NOTHING,
                       "Funny object type.  Destroyed.");
      destroy_obj(evaluation, NOTHING, i);
    }

    if (full_check) {

      /*
       * Check for wizards
       */

      if (is_wizard(evaluation->world->database, i)) {
        if (is_player(evaluation->world->database, i)) {
          log_simple_error(evaluation, i, NOTHING, "Player is a WIZARD.");
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
 *       Location GOING                                  - skip it.
 *       Location not a PLAYER, ROOM, or THING           - skip it.
 *       Location already visited                        - skip it.
 *       Exit/next pointer not in database               - NULL it.
 *       Member is not an EXIT                           - terminate chain.
 *       Member is GOING                                 - destroy exit.
 *       Member already checked (is in another list)     - terminate chain.
 *       Member in another chain (recursive check)       - terminate chain.
 *       Location of member is not specified location    - reset it.
 */
static void check_loc_exits(EvaluationContext *evaluation, DbRef loc,
                            bool full_check) {
  DbRef exit, back, temp, exitloc, dest;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  /*
   * Only check players, rooms, and things that aren't GOING
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

      Log_pointer_err(evaluation, back, loc, NOTHING, exit, "Exit list",
                      "is invalid.  List nulled.");
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

      Log_pointer_err(evaluation, back, loc, NOTHING, exit, "Exitlist member",
                      "is not an exit.  List terminated.");
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
      destroy_obj(evaluation, NOTHING, exit);
      exit = temp;
      continue;
    } else if (is_marked(evaluation->world->database, exit)) {

      /*
       * Already in another list - terminate chain
       */

      Log_pointer_err(evaluation, back, loc, NOTHING, exit, "Exitlist member",
                      "is in another exitlist.  Cleared.");
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

      Log_pointer_err(evaluation, back, loc, NOTHING, exit, "Destination",
                      "is invalid.  Cleared.");
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

        Log_pointer_err(evaluation, back, loc, NOTHING, exit, "",
                        "is in another exitlist.  List terminated.");
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

        Log_header_err(evaluation, exit, loc, exitloc, 1,
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
  return;
}

static void check_exit_chains(EvaluationContext *evaluation, bool full_check) {
  DbRef i;

  unmark_all(evaluation->world->database);
  DO_WHOLE_DB(evaluation->world->database, i)
  check_loc_exits(evaluation, i, full_check);
  DO_WHOLE_DB(evaluation->world->database, i) {
    if (is_exit(evaluation->world->database, i) &&
        !is_marked(evaluation->world->database, i)) {
      log_simple_error(evaluation, i, NOTHING,
                       "Disconnected exit.  Destroyed.");
      destroy_obj(evaluation, NOTHING, i);
    }
  }
}

/**
 * check_misplaced_obj, check_loc_contents, check_contents_chains: Validate
 * the contents chains of objects and attempt to correct problems.  The
 * following errors are found and corrected:
 *       Location not in database                        - skip it.
 *       Location GOING                                  - skip it.
 *       Location not a PLAYER, ROOM, or THING           - skip it.
 *       Location already visited                        - skip it.
 *       Contents/next pointer not in database           - NULL it.
 *       Member is not an PLAYER or THING                - terminate chain.
 *       Member is GOING                                 - destroy exit.
 *       Member already checked (is in another list)     - terminate chain.
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

    Log_pointer_err(evaluation, back, loc, NOTHING, *obj, "",
                    "is in another contents list.  Cleared.");
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

    Log_header_err(evaluation, *obj, loc,
                   game_object_contents(evaluation->world->database, *obj), 1,
                   "Location", "is invalid.  Reset.");
    game_object_set_contents(evaluation->world->database, *obj, loc);
  }
  return;
}

static void check_loc_contents(EvaluationContext *evaluation, DbRef loc,
                               bool full_check) {
  DbRef obj, back, temp;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  /*
   * Only check players, rooms, and things that aren't GOING
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

      Log_pointer_err(evaluation, back, loc, NOTHING, obj, "Contents list",
                      "is invalid.  Cleared.");
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

      Log_pointer_err(evaluation, back, loc, NOTHING, obj, "",
                      "is not a player or thing.  Cleared.");
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, NOTHING);
      } else {
        game_object_set_contents(evaluation->world->database, loc, NOTHING);
      }
      obj = NOTHING;
    } else if (is_going(evaluation->world->database, obj) &&
               (typeof_obj(evaluation->world->database, obj) == TYPE_GARBAGE)) {

      /*
       * Going - silently filter out
       */

      temp = game_object_next(evaluation->world->database, obj);
      if (back != NOTHING) {
        game_object_set_next(evaluation->world->database, back, temp);
      } else {
        game_object_set_contents(evaluation->world->database, loc, temp);
      }
      destroy_obj(evaluation, NOTHING, obj);
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
          log_simple_error(evaluation, obj, loc,
                           "Nonwizard object inside wizard.");
        }
      }
      mark(evaluation->world->database, obj);
      back = obj;
      obj = game_object_next(evaluation->world->database, obj);
    }
  }
  return;
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
    log_simple_error(evaluation, i,
                     game_object_location(evaluation->world->database, i),
                     "Orphaned object, moved home.");
    ZAP_LOC(evaluation->world->database, i);
    move_via_generic(evaluation, i, HOME, NOTHING, 0);
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
      log_simple_error(evaluation, i, NOTHING, "Floating room.");
    }
  }
}

/**
 * Perform a database consistency check and clean up damage.
 */
void database_check(EvaluationContext *evaluation, DbRef player, int key) {
  const bool full_check = (key & DBCK_FULL) != 0;

  make_freelist(evaluation->world->database);
  check_dead_refs(evaluation, full_check);
  check_exit_chains(evaluation, full_check);
  check_contents_chains(evaluation, full_check);
  check_floating(evaluation);
  purge_going(evaluation, full_check);

  if (player != NOTHING) {
    if (!is_quiet(evaluation->world->database, player))
      notify(evaluation, player, "Done.");
  }
}

void do_dbck(CommandInvocation *invocation) {
  database_check(&invocation->context->evaluation, invocation->player,
                 invocation->key);
}
