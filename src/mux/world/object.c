/*
 * object.c - low-level object manipulation routines
 */

#include "mux/world/object.h"

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"

#define IS_CLEAN(i)                                                            \
  (is_flag_set(i, TYPE_GARBAGE, GOING) && (obj_location(i) == NOTHING) &&      \
   (obj_contents(i) == NOTHING) && (obj_exits(i) == NOTHING) &&                \
   (obj_next(i) == NOTHING) && (obj_owner(i) == GOD))

#define ZAP_LOC(i)                                                             \
  {                                                                            \
    s_location(i, NOTHING);                                                    \
    s_next(i, NOTHING);                                                        \
  }

static int check_type;
extern int boot_off(DbRef player, char *message);

/**
 * Log_pointer_err, Log_header_err, Log_simple_damage: Write errors to the
 * log file.
 */
static void Log_pointer_err(DbRef prior, DbRef obj, DbRef loc, DbRef ref,
                            const char *reftype, const char *errtype) {
  STARTLOG(LOG_PROBLEMS, "OBJ", "DAMAG") {
    log_type_and_name(obj);
    if (loc != NOTHING) {
      log_text((char *)" in ");
      log_type_and_name(loc);
    }
    log_text((char *)": ");
    if (prior == NOTHING) {
      log_text((char *)reftype);
    } else {
      log_text((char *)"Next pointer");
    }
    log_text((char *)" ");
    log_type_and_name(ref);
    log_text((char *)" ");
    log_text((char *)errtype);
    ENDLOG;
  }
}

static void Log_header_err(DbRef obj, DbRef loc, DbRef val, int is_object,
                           const char *valtype, const char *errtype) {
  STARTLOG(LOG_PROBLEMS, "OBJ", "DAMAG") {
    log_type_and_name(obj);
    if (loc != NOTHING) {
      log_text((char *)" in ");
      log_type_and_name(loc);
    }
    log_text((char *)": ");
    log_text((char *)valtype);
    log_text((char *)" ");
    if (is_object)
      log_type_and_name(val);
    else
      log_number(val);
    log_text((char *)" ");
    log_text((char *)errtype);
    ENDLOG;
  }
}

static void log_simple_error(DbRef obj, DbRef loc, const char *errtype) {
  STARTLOG(LOG_PROBLEMS, "OBJ", "DAMAG") {
    log_type_and_name(obj);
    if (loc != NOTHING) {
      log_text((char *)" in ");
      log_type_and_name(loc);
    }
    log_text((char *)": ");
    log_text((char *)errtype);
    ENDLOG;
  }
}

/**
 * start_home, default_home, can_set_home, new_home, clone_home:
 * Routines for validating and determining homes.
 */
DbRef start_home(void) {
  if (mudconf.start_home != NOTHING)
    return mudconf.start_home;
  return mudconf.start_room;
}

DbRef default_home(void) {
  if (mudconf.default_home != NOTHING)
    return mudconf.default_home;
  if (mudconf.start_home != NOTHING)
    return mudconf.start_home;
  return mudconf.start_room;
}

int can_set_home(DbRef player, DbRef thing, DbRef home) {
  if (!is_good_obj(player) || !is_good_obj(home) || (thing == home))
    return 0;

  switch (typeof_obj(home)) {
  case TYPE_PLAYER:
  case TYPE_ROOM:
  case TYPE_THING:
    if (is_going(home))
      return 0;
    if (is_controls(player, home))
      return 1;
  }
  return 0;
}

DbRef new_home(DbRef player) {
  DbRef loc;

  loc = obj_location(player);
  if (can_set_home(obj_owner(player), player, loc))
    return loc;
  loc = obj_home(obj_owner(player));
  if (can_set_home(obj_owner(player), player, loc))
    return loc;
  return default_home();
}

DbRef clone_home(DbRef player, DbRef thing) {
  DbRef loc;

  loc = obj_home(thing);
  if (can_set_home(obj_owner(player), player, loc))
    return loc;
  return new_home(player);
}

/**
 * Build a freelist
 */
static void make_freelist(void) {
  DbRef i;

  mudstate.freelist = NOTHING;
  DO_WHOLE_DB_REV(i) {
    if (IS_CLEAN(i)) {
      s_link(i, mudstate.freelist);
      mudstate.freelist = i;
    }
  }
}

/**
 * Create an object of the indicated type.
 */
DbRef create_obj(DbRef player, int objtype, char *name) {
  DbRef obj, owner;
  int okname = 0, self_owned, require_inherit;
  Flag f1, f2, f3;
  time_t tt;
  char *buff;

  self_owned = 0;
  require_inherit = 0;

  switch (objtype) {
  case TYPE_ROOM:
    f1 = mudconf.room_flags.word1;
    f2 = mudconf.room_flags.word2;
    f3 = mudconf.room_flags.word3;
    okname = ok_name(name);
    break;
  case TYPE_THING:
    f1 = mudconf.thing_flags.word1;
    f2 = mudconf.thing_flags.word2;
    f3 = mudconf.thing_flags.word3;
    okname = ok_name(name);
    break;
  case TYPE_EXIT:
    f1 = mudconf.exit_flags.word1;
    f2 = mudconf.exit_flags.word2;
    f3 = mudconf.exit_flags.word3;
    okname = ok_name(name);
    break;
  case TYPE_PLAYER:
    if (player != NOTHING) {
      f1 = mudconf.robot_flags.word1;
      f2 = mudconf.robot_flags.word2;
      f3 = mudconf.robot_flags.word3;
      require_inherit = 1;
    } else {
      f1 = mudconf.player_flags.word1;
      f2 = mudconf.player_flags.word2;
      f3 = mudconf.player_flags.word3;
      self_owned = 1;
    }
    buff = munge_space(name);
    if (!badname_check(buff)) {
      notify(player, "That name is not allowed.");
      free_lbuf(buff);
      return NOTHING;
    }
    if (*buff) {
      okname = ok_player_name(buff);
      if (!okname) {
        notify(player, "That's a silly name for a player.");
        free_lbuf(buff);
        return NOTHING;
      }
    }
    if (okname) {
      okname = (lookup_player(NOTHING, buff, 0) == NOTHING);
      if (!okname) {
        notify_printf(player, "The name %s is already taken.", name);
        free_lbuf(buff);
        return NOTHING;
      }
    }
    free_lbuf(buff);
    break;
  default:
    LOG_SIMPLE(LOG_BUGS, "BUG", "OTYPE",
               tprintf("Bad object type in create_obj: %d.", objtype));
    return NOTHING;
  }

  if (!self_owned) {
    if (!is_good_obj(player))
      return NOTHING;
    owner = obj_owner(player);
    if (!is_good_obj(owner))
      return NOTHING;
  } else {
    owner = NOTHING;
  }

  if (require_inherit) {
    if (!is_inherits(player)) {
      notify(player, "Permission denied.");
      return NOTHING;
    }
  }
  /*
   * Get the first object from the freelist. If the object is not
   * clean, discard the remainder of the freelist and go get a
   * completely new object.
   */

  obj = NOTHING;
  if (mudstate.freelist != NOTHING) {
    obj = mudstate.freelist;
    if (is_good_obj(obj) && IS_CLEAN(obj)) {
      mudstate.freelist = obj_link(obj);
    } else {
      LOG_SIMPLE(LOG_PROBLEMS, "FRL", "DAMAG",
                 tprintf("Freelist damaged, bad object #%ld.", obj));
      obj = NOTHING;
      mudstate.freelist = NOTHING;
    }
  }
  if (obj == NOTHING) {
    obj = mudstate.db_top;
    db_grow(mudstate.db_top + 1);
  }
  attribute_free(obj); // Just in case...

  /*
   * Set things up according to the object type
   */

  s_location(obj, NOTHING);
  s_contents(obj, NOTHING);
  s_exits(obj, NOTHING);
  s_next(obj, NOTHING);
  s_link(obj, NOTHING);

  if (objtype == TYPE_ROOM && mudconf.room_parent > 0)
    s_parent(obj, mudconf.room_parent);
  else if (objtype == TYPE_EXIT && mudconf.exit_parent > 0)
    s_parent(obj, mudconf.exit_parent);
  else if (objtype == TYPE_PLAYER && mudconf.player_parent > 0)
    s_parent(obj, mudconf.player_parent);
  else
    s_parent(obj, NOTHING);

  if (objtype == TYPE_PLAYER && mudconf.player_zone > 0)
    s_zone(obj, mudconf.player_zone);
  else
    s_zone(obj, obj_zone(player));

  s_flags(obj, objtype | f1);
  s_flags2(obj, f2);
  s_flags3(obj, f3);
  s_owner(obj, (self_owned ? obj : owner));
  unmark(obj);
  buff = munge_space((char *)name);
  object_name_set(obj, buff);
  free_lbuf(buff);

  if (objtype == TYPE_PLAYER) {
    time(&tt);
    buff = (char *)ctime(&tt);
    buff[strlen(buff) - 1] = '\0';
    attribute_add_raw(obj, A_LAST, buff);

    add_player_name(obj, Name(obj));
  }
  make_freelist();
  return obj;
}

/**
 * Destroy an object. Assumes it has already been removed from
 * all lists and has no contents or exits.
 */
void destroy_obj(DbRef player, DbRef obj) {
  DbRef owner;
  int good_owner;
  AttributeStack *sp, *next;
  char *tname;

  if (!is_good_obj(obj))
    return;

  /*
   * Validate the owner
   */

  owner = obj_owner(obj);
  good_owner = is_good_owner(owner);

  /*
   * Halt any pending commands (waiting or semaphore)
   */
  if (halt_que(NOTHING, obj) > 0) {
    if (good_owner && !is_quiet(obj) && !is_quiet(owner)) {
      notify(owner, "Halted.");
    }
  }
  nfy_que(obj, 0, NFY_DRAIN, 0);

  if ((player != NOTHING) && !is_quiet(player)) {
    if (good_owner && obj_owner(player) != owner) {
      if (owner == obj) {
        notify_printf(player, "Destroyed. %s(#%ld)", Name(obj), obj);
      } else {
        tname = alloc_sbuf("destroy_obj");
        StringCopy(tname, Name(owner));
        notify_printf(player, "Destroyed. %s's %s(#%ld)", tname, Name(obj),
                      obj);
        free_sbuf(tname);
      }
    } else if (!is_quiet(obj)) {
      notify(player, "Destroyed.");
    }
  }

  attribute_free(obj);
  object_name_set(obj, "Garbage");
  s_flags(obj, (TYPE_GARBAGE | GOING));
  s_flags2(obj, 0);
  s_flags3(obj, 0);
  s_powers(obj, 0);
  s_powers2(obj, 0);
  s_location(obj, NOTHING);
  s_contents(obj, NOTHING);
  s_exits(obj, NOTHING);
  s_next(obj, NOTHING);
  s_link(obj, NOTHING);
  s_owner(obj, GOD);
  s_parent(obj, NOTHING);
  s_zone(obj, NOTHING);

  /*
   * Clear the stack
   */
  for (sp = obj_stack(obj); sp != nullptr; sp = next) {
    next = sp->next;
    free_lbuf(sp->data);
    free(sp);
  }

  s_stack(obj, nullptr);

  if (mudconf.have_comsys)
    toast_player(obj);

  make_freelist();
  return;
}

/**
 * Get rid of KEY contents of object.
 */
void divest_object(DbRef thing) {
  DbRef curr, temp;

  SAFE_DOLIST(curr, temp, obj_contents(thing)) {
    if (!is_controls(thing, curr) && has_location(curr) && has_key_flag(curr)) {
      move_via_generic(curr, HOME, NOTHING, 0);
    }
  }
}

/**
 * Empties the contents of a GOING object.
 */
void empty_obj(DbRef obj) {
  DbRef targ, next;

  /*
   * Send the contents home
   */

  SAFE_DOLIST(targ, next, obj_contents(obj)) {
    if (!has_location(targ)) {
      log_simple_error(targ, obj,
                       "Funny object type in contents list of GOING location. "
                       "Flush terminated.");
      break;
    } else if (obj_location(targ) != obj) {
      Log_header_err(targ, obj, obj_location(targ), 1, "Location",
                     "indicates object really in another location during "
                     "cleanup of GOING location.  Flush terminated.");
      break;
    } else {
      ZAP_LOC(targ);
      if (obj_home(targ) == obj) {
        s_home(targ, new_home(targ));
      }
      move_via_generic(targ, HOME, NOTHING, 0);
      divest_object(targ);
    }
  }

  /*
   * Destroy the exits
   */

  SAFE_DOLIST(targ, next, obj_exits(obj)) {
    if (!is_exit(targ)) {
      log_simple_error(
          targ, obj,
          "Funny object type in exit list of GOING location. Flush "
          "terminated.");
      break;
    } else if (obj_exits(targ) != obj) {
      Log_header_err(targ, obj, obj_exits(targ), 1, "Location",
                     "indicates exit really in another location during cleanup "
                     "of GOING location.  Flush terminated.");
      break;
    } else {
      destroy_obj(NOTHING, targ);
    }
  }
}

/**
 * Destroys an exit.
 */
void destroy_exit(DbRef exit) {
  DbRef loc;

  loc = obj_exits(exit);
  s_exits(loc, remove_first(obj_exits(loc), exit));
  destroy_obj(NOTHING, exit);
}

/**
 * Destroys a thing.
 */
void destroy_thing(DbRef thing) {
  move_via_generic(thing, NOTHING, obj_owner(thing), 0);
  empty_obj(thing);
  destroy_obj(NOTHING, thing);
}

/**
 * Destroys a player.
 */
void destroy_player(DbRef victim) {
  DbRef aowner, player;
  int count;
  long aflags;
  char *buf;

  /*
   * Bye bye...
   */
  player = (DbRef)atoi(attribute_get_raw(victim, A_DESTROYER));
  toast_player(victim);
  boot_off(victim, (char *)"You have been destroyed!");
  halt_que(victim, NOTHING);
  count = chown_all(victim, player);

  /*
   * Remove the name from the name hash table
   */

  delete_player_name(victim, Name(victim));
  buf = attribute_parent_get(victim, A_ALIAS, &aowner, &aflags);
  delete_player_name(victim, buf);
  free_lbuf(buf);

  move_via_generic(victim, NOTHING, player, 0);
  destroy_obj(NOTHING, victim);
  notify_quiet(player, tprintf("(%d objects @chowned to you)", count));
}

/**
 * Purges a GOING object.
 */
static void purge_going(void) {
  DbRef i;

  DO_WHOLE_DB(i) {
    if (!is_going(i))
      continue;

    switch (typeof_obj(i)) {
    case TYPE_PLAYER:
      destroy_player(i);
      break;
    case TYPE_ROOM:

      /*
       * Room scheduled for destruction... do it
       */

      empty_obj(i);
      destroy_obj(NOTHING, i);
      break;
    case TYPE_THING:
      destroy_thing(i);
      break;
    case TYPE_EXIT:
      destroy_exit(i);
      break;
    case TYPE_GARBAGE:
      break;
    default:

      /*
       * Something else... How did this happen?
       */

      log_simple_error(i, NOTHING,
                       "GOING object with unexpected type.  Destroyed.");
      destroy_obj(NOTHING, i);
    }
  }
}

/**
 * Look for references to GOING or illegal objects.
 */
static void check_dead_refs(void) {
  DbRef targ, owner, i, j;
  long aflags;
  int dirty;
  char *str;
  FWDLIST *fp;

  DO_WHOLE_DB(i) {

    /*
     * Check the parent
     */

    targ = obj_parent(i);
    if (is_good_obj(targ)) {
      if (is_going(targ)) {
        s_parent(i, NOTHING);
        owner = obj_owner(i);

        if (is_good_owner(owner) && !is_quiet(i) && !is_quiet(owner)) {
          notify_printf(owner, "Parent cleared on %s(#%ld)", Name(i), i);
        }
      }
    } else if (targ != NOTHING) {
      Log_header_err(i, obj_location(i), targ, 1, "Parent",
                     "is invalid.  Cleared.");
      s_parent(i, NOTHING);
    }
    /*
     * Check the zone
     */

    targ = obj_zone(i);
    if (is_good_obj(targ)) {
      if (is_going(targ)) {
        s_zone(i, NOTHING);
        owner = obj_owner(i);
        if (is_good_owner(owner) && !is_quiet(i) && !is_quiet(owner)) {
          notify_printf(owner, "Zone cleared on %s(#%ld)", Name(i), i);
        }
      }
    } else if (targ != NOTHING) {
      Log_header_err(i, obj_location(i), targ, 1, "Zone",
                     "is invalid. Cleared.");
      s_zone(i, NOTHING);
    }
    switch (typeof_obj(i)) {
    case TYPE_PLAYER:
    case TYPE_THING:

      if (is_going(i))
        break;

      /*
       * Check the home
       */

      targ = obj_home(i);
      if (is_good_obj(targ)) {
        if (is_going(targ)) {
          s_home(i, new_home(i));
          owner = obj_owner(i);
          if (is_good_owner(owner) && !is_quiet(i) && !is_quiet(owner)) {
            notify_printf(owner, "Home reset on %s(#%ld)", Name(i), i);
          }
        }
      } else if (targ != NOTHING) {
        Log_header_err(i, obj_location(i), targ, 1, "Home",
                       "is invalid.  Cleared.");
        s_home(i, new_home(i));
      }
      /*
       * Check the location
       */

      targ = obj_location(i);
      if (!is_good_obj(targ)) {
        Log_pointer_err(NOTHING, i, NOTHING, targ, "Location",
                        "is invalid.  Moved to home.");
        ZAP_LOC(i);
        move_object(i, HOME);
      }
      /*
       * Check for self-referential obj_next()
       */

      if (obj_next(i) == i) {
        log_simple_error(i, NOTHING, "Next points to self.  Next cleared.");
        s_next(i, NOTHING);
      }
      break;
    case TYPE_ROOM:

      /*
       * Check the dropto
       */

      targ = obj_dropto(i);
      if (is_good_obj(targ)) {
        if (is_going(targ)) {
          s_dropto(i, NOTHING);
          owner = obj_owner(i);
          if (is_good_owner(owner) && !is_quiet(i) && !is_quiet(owner)) {
            notify_printf(owner, "Dropto removed from %s(#%ld)", Name(i), i);
          }
        }
      } else if ((targ != NOTHING) && (targ != HOME)) {
        Log_header_err(i, NOTHING, targ, 1, "Dropto", "is invalid.  Cleared.");
        s_dropto(i, NOTHING);
      }
      if (check_type & DBCK_FULL) {

        /*
         * NEXT should be null
         */

        if (obj_next(i) != NOTHING) {
          Log_header_err(i, NOTHING, obj_next(i), 1, "Next pointer",
                         "should be NOTHING.  Reset.");
          s_next(i, NOTHING);
        }
        /*
         * LINK should be null
         */

        if (obj_link(i) != NOTHING) {
          Log_header_err(i, NOTHING, obj_link(i), 1, "Link pointer ",
                         "should be NOTHING.  Reset.");
          s_link(i, NOTHING);
        }
      }
      break;
    case TYPE_EXIT:

      /*
       * If it points to something GOING, set it going
       */

      targ = obj_location(i);
      if (is_good_obj(targ)) {
        if (is_going(targ)) {
          s_going(i);
        }
      } else if (targ == HOME) {
        /*
         * null case, HOME is always valid
         */
      } else if (targ != NOTHING) {
        Log_header_err(i, obj_exits(i), targ, 1, "Destination",
                       "is invalid.  Exit destroyed.");
        s_going(i);
      } else {
        if (!has_contents(targ)) {
          Log_header_err(i, obj_exits(i), targ, 1, "Destination",
                         "is not a valid type.  Exit destroyed.");
          s_going(i);
        }
      }

      /*
       * Check for self-referential obj_next()
       */

      if (obj_next(i) == i) {
        log_simple_error(i, NOTHING, "Next points to self.  Next cleared.");
        s_next(i, NOTHING);
      }
      if (check_type & DBCK_FULL) {

        /*
         * CONTENTS should be null
         */

        if (obj_contents(i) != NOTHING) {
          Log_header_err(i, obj_exits(i), obj_contents(i), 1, "Contents",
                         "should be NOTHING.  Reset.");
          s_contents(i, NOTHING);
        }
        /*
         * LINK should be null
         */

        if (obj_link(i) != NOTHING) {
          Log_header_err(i, obj_exits(i), obj_link(i), 1, "Link",
                         "should be NOTHING.  Reset.");
          s_link(i, NOTHING);
        }
      }
      break;
    case TYPE_GARBAGE:
      break;
    default:

      /*
       * Funny object type, destroy it
       */

      log_simple_error(i, NOTHING, "Funny object type.  Destroyed.");
      destroy_obj(NOTHING, i);
    }

    /*
     * Check forwardlist
     */

    dirty = 0;
    fp = fwdlist_get(i);
    if (fp) {
      for (j = 0; j < fp->count; j++) {
        targ = fp->data[j];
        if (is_good_obj(targ) && is_going(targ)) {
          fp->data[j] = NOTHING;
          dirty = 1;
        } else if (!is_good_obj(targ) && (targ != NOTHING)) {
          fp->data[j] = NOTHING;
          dirty = 1;
        }
      }
    }
    if (dirty) {
      str = alloc_lbuf("purge_going");
      (void)fwdlist_rewrite(fp, str);
      attribute_get_info(i, A_FORWARDLIST, &owner, &aflags);
      attribute_add(i, A_FORWARDLIST, str, owner, aflags);
      free_lbuf(str);
    }
    /*
     * Check owner
     */

    owner = obj_owner(i);
    if (!is_good_obj(owner)) {
      Log_header_err(i, NOTHING, owner, 1, "Owner", "is invalid.  Set to GOD.");
      owner = GOD;
      s_owner(i, owner);
      halt_que(NOTHING, i);
      s_halted(i);
    } else if (check_type & DBCK_FULL) {
      if (is_going(owner)) {
        Log_header_err(i, NOTHING, owner, 1, "Owner",
                       "is set GOING.  Set to GOD.");
        s_owner(i, owner);
        halt_que(NOTHING, i);
        s_halted(i);
      } else if (!is_owns_others(owner)) {
        Log_header_err(i, NOTHING, owner, 1, "Owner",
                       "is not a valid owner type.");
      } else if (is_player(i) && (owner != i)) {
        Log_header_err(i, NOTHING, owner, 1, "Player",
                       "is the owner instead of the player.");
      }
    }
    if (check_type & DBCK_FULL) {

      /*
       * Check for wizards
       */

      if (is_wizard(i)) {
        if (is_player(i)) {
          log_simple_error(i, NOTHING, "Player is a WIZARD.");
        }
        if (!is_wizard(obj_owner(i))) {
          Log_header_err(i, NOTHING, obj_owner(i), 1, "Owner",
                         "of a WIZARD object is not a wizard");
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
static void check_loc_exits(DbRef loc) {
  DbRef exit, back, temp, exitloc, dest;

  if (!is_good_obj(loc))
    return;

  /*
   * Only check players, rooms, and things that aren't GOING
   */

  if (is_exit(loc) || is_going(loc))
    return;

  /*
   * If marked, we've checked here already
   */

  if (is_marked(loc))
    return;
  mark(loc);

  /*
   * Check all the exits
   */

  back = NOTHING;
  exit = obj_exits(loc);
  while (exit != NOTHING) {

    exitloc = NOTHING;
    dest = NOTHING;

    if (is_good_obj(exit)) {
      exitloc = obj_exits(exit);
      dest = obj_location(exit);
    }
    if (!is_good_obj(exit)) {

      /*
       * A bad pointer - terminate chain
       */

      Log_pointer_err(back, loc, NOTHING, exit, "Exit list",
                      "is invalid.  List nulled.");
      if (back != NOTHING) {
        s_next(back, NOTHING);
      } else {
        s_exits(loc, NOTHING);
      }
      exit = NOTHING;
    } else if (!is_exit(exit)) {

      /*
       * Not an exit - terminate chain
       */

      Log_pointer_err(back, loc, NOTHING, exit, "Exitlist member",
                      "is not an exit.  List terminated.");
      if (back != NOTHING) {
        s_next(back, NOTHING);
      } else {
        s_exits(loc, NOTHING);
      }
      exit = NOTHING;
    } else if (is_going(exit)) {

      /*
       * Going - silently filter out
       */

      temp = obj_next(exit);
      if (back != NOTHING) {
        s_next(back, temp);
      } else {
        s_exits(loc, temp);
      }
      destroy_obj(NOTHING, exit);
      exit = temp;
      continue;
    } else if (is_marked(exit)) {

      /*
       * Already in another list - terminate chain
       */

      Log_pointer_err(back, loc, NOTHING, exit, "Exitlist member",
                      "is in another exitlist.  Cleared.");
      if (back != NOTHING) {
        s_next(back, NOTHING);
      } else {
        s_exits(loc, NOTHING);
      }
      exit = NOTHING;
    } else if (!is_good_obj(dest) && (dest != HOME) && (dest != NOTHING)) {

      /*
       * Destination is not in the db.  Null it.
       */

      Log_pointer_err(back, loc, NOTHING, exit, "Destination",
                      "is invalid.  Cleared.");
      s_location(exit, NOTHING);

    } else if (exitloc != loc) {

      /*
       * Exit thinks it's in another place. Check the
       * exitlist there and see if it contains this
       * exit. If it does, then our exitlist
       * somehow pointed into the middle of their
       * exitlist. If not, assume we own the exit.
       */

      check_loc_exits(exitloc);
      if (is_marked(exit)) {

        /*
         * It's in the other list, give it up
         */

        Log_pointer_err(back, loc, NOTHING, exit, "",
                        "is in another exitlist.  List terminated.");
        if (back != NOTHING) {
          s_next(back, NOTHING);
        } else {
          s_exits(loc, NOTHING);
        }
        exit = NOTHING;
      } else {

        /*
         * Not in the other list, assume in ours
         */

        Log_header_err(exit, loc, exitloc, 1, "Not on chain for location",
                       "Reset.");
        s_exits(exit, loc);
      }
    }
    if (exit != NOTHING) {

      /*
       * All OK (or all was made OK)
       */

      if (check_type & DBCK_FULL) {

        /*
         * Make sure exit owner owns at least one of
         * * * * * the source or destination.  Just *
         * warn * if * * he doesn't.
         */

        temp = obj_owner(exit);
        if ((temp != obj_owner(loc)) &&
            (temp != obj_owner(obj_location(exit)))) {
          Log_header_err(exit, loc, temp, 1, "Owner",
                         "does not own either the source or destination.");
        }
      }
      mark(exit);
      back = exit;
      exit = obj_next(exit);
    }
  }
  return;
}

static void check_exit_chains(void) {
  DbRef i;

  Unmark_all(i);
  DO_WHOLE_DB(i)
  check_loc_exits(i);
  DO_WHOLE_DB(i) {
    if (is_exit(i) && !is_marked(i)) {
      log_simple_error(i, NOTHING, "Disconnected exit.  Destroyed.");
      destroy_obj(NOTHING, i);
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

static void check_loc_contents(DbRef);

static void check_misplaced_obj(DbRef *obj, DbRef back, DbRef loc) {
  /*
   * Object thinks it's in another place. Check the contents list
   * there and see if it contains this object. If it does, then
   * our contents list somehow pointed into the middle of their
   * contents list and we should truncate our list. If not,
   * assume we own the object.
   */

  if (!is_good_obj(*obj))
    return;
  loc = obj_location(*obj);
  unmark(*obj);
  if (is_good_obj(loc)) {
    check_loc_contents(loc);
  }
  if (is_marked(*obj)) {

    /*
     * It's in the other list, give it up
     */

    Log_pointer_err(back, loc, NOTHING, *obj, "",
                    "is in another contents list.  Cleared.");
    if (back != NOTHING) {
      s_next(back, NOTHING);
    } else {
      s_contents(loc, NOTHING);
    }
    *obj = NOTHING;
  } else {
    /*
     * Not in the other list, assume in ours
     */

    Log_header_err(*obj, loc, obj_contents(*obj), 1, "Location",
                   "is invalid.  Reset.");
    s_contents(*obj, loc);
  }
  return;
}

static void check_loc_contents(DbRef loc) {
  DbRef obj, back, temp;

  if (!is_good_obj(loc))
    return;

  /*
   * Only check players, rooms, and things that aren't GOING
   */

  if (is_exit(loc) || is_going(loc))
    return;

  /*
   * Check all the exits
   */

  back = NOTHING;
  obj = obj_contents(loc);
  while (obj != NOTHING) {
    if (!is_good_obj(obj)) {

      /*
       * A bad pointer - terminate chain
       */

      Log_pointer_err(back, loc, NOTHING, obj, "Contents list",
                      "is invalid.  Cleared.");
      if (back != NOTHING) {
        s_next(back, NOTHING);
      } else {
        s_contents(loc, NOTHING);
      }
      obj = NOTHING;
    } else if (!has_location(obj)) {

      /*
       * Not a player or thing - terminate chain
       */

      Log_pointer_err(back, loc, NOTHING, obj, "",
                      "is not a player or thing.  Cleared.");
      if (back != NOTHING) {
        s_next(back, NOTHING);
      } else {
        s_contents(loc, NOTHING);
      }
      obj = NOTHING;
    } else if (is_going(obj) && (typeof_obj(obj) == TYPE_GARBAGE)) {

      /*
       * Going - silently filter out
       */

      temp = obj_next(obj);
      if (back != NOTHING) {
        s_next(back, temp);
      } else {
        s_contents(loc, temp);
      }
      destroy_obj(NOTHING, obj);
      obj = temp;
      continue;
    } else if (is_marked(obj)) {

      /*
       * Already visited - either truncate or ignore
       */

      if (obj_location(obj) != loc) {

        /*
         * Location wrong - either truncate or fix
         */

        check_misplaced_obj(&obj, back, loc);
      } else {

        /*
         * Location right - recursive contents
         */
      }
    } else if (obj_location(obj) != loc) {

      /*
       * Location wrong - either truncate or fix
       */

      check_misplaced_obj(&obj, back, loc);
    }
    if (obj != NOTHING) {

      /*
       * All OK (or all was made OK)
       */

      if (check_type & DBCK_FULL) {

        /*
         * Check for wizard command-handlers inside *
         *
         * *  * *  * * nonwiz. Just warn if we find
         * one.
         */

        if (is_wizard(obj) && !is_wizard(loc)) {
          if (has_commands(obj)) {
            log_simple_error(
                obj, loc, "Wizard command handling object inside nonwizard.");
          }
        }
        /*
         * Check for nonwizard objects inside wizard
         * * * * * objects.
         */

        if (is_wizard(loc) && !is_wizard(obj) && !is_wizard(obj_owner(obj))) {
          log_simple_error(obj, loc, "Nonwizard object inside wizard.");
        }
      }
      mark(obj);
      back = obj;
      obj = obj_next(obj);
    }
  }
  return;
}

static void check_contents_chains(void) {
  DbRef i;

  Unmark_all(i);
  DO_WHOLE_DB(i)
  check_loc_contents(i);
  DO_WHOLE_DB(i)
  if (!is_going(i) && !is_marked(i) && has_location(i)) {
    log_simple_error(i, obj_location(i), "Orphaned object, moved home.");
    ZAP_LOC(i);
    move_via_generic(i, HOME, NOTHING, 0);
  }
}

/**
 * mark_place, check_floating: Look for floating rooms not set FLOATING.
 */
static void mark_place(DbRef loc) {
  DbRef exit;

  /*
   * If already marked, exit.  Otherwise set marked.
   */

  if (!is_good_obj(loc))
    return;
  if (is_marked(loc))
    return;
  mark(loc);

  /*
   * Visit all places you can get to via exits from here.
   */

  for (exit = obj_exits(loc); exit != NOTHING; exit = obj_next(exit)) {
    if (is_good_obj(obj_location(exit)))
      mark_place(obj_location(exit));
  }
}

static void check_floating(void) {
  DbRef owner, i;

  /*
   * Mark everyplace you can get to via exits from the starting room
   */

  Unmark_all(i);
  mark_place(mudconf.start_room);

  /*
   * Look for rooms not marked and not set FLOATING
   */

  DO_WHOLE_DB(i) {
    if (is_room(i) && !is_floating(i) && !is_going(i) && !is_marked(i)) {
      owner = obj_owner(i);
      if (is_good_owner(owner)) {
        notify_printf(owner, "You own a floating room: %s(#%ld)", Name(i), i);
      }
    }
  }
}

/**
 * Perform a database consistency check and clean up damage.
 */
void do_dbck(DbRef player, DbRef cause, int key) {
  check_type = key;
  make_freelist();
  check_dead_refs();
  check_exit_chains();
  check_contents_chains();
  check_floating();
  purge_going();

  if (player != NOTHING) {
    if (!is_quiet(player))
      notify(player, "Done.");
  }
}
