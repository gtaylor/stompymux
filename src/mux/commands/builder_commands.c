/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include "mux/commands/command.h"

#include "p.glue.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"
#include "mux/world/object.h"
#include "mux/world/object_set.h"
#include "mux/world/walkdb.h"

extern NameTable indiv_attraccess_nametab[];

/*
 * ---------------------------------------------------------------------------
 * * parse_linkable_room: Get a location to link to.
 */

static DbRef parse_linkable_room(EvaluationContext *evaluation,
                                 MatchContext *match, DbRef player,
                                 char *room_name) {
  DbRef room;

  init_match(match, player, room_name, NOTYPE);
  match_everything(match, MAT_NO_EXITS | MAT_NUMERIC | MAT_HOME);
  room = match_result(match);

  /*
   * HOME is always linkable
   */

  if (room == HOME)
    return HOME;

  /*
   * Make sure we can link to it
   */

  if (!is_good_obj(evaluation->world->database, room)) {
    notify_quiet(evaluation, player, "That's not a valid object.");
    return NOTHING;
  } else if (!has_contents(evaluation->world->database, room) ||
             !is_linkable(evaluation->world->database, player, room)) {
    notify_quiet(evaluation, player, "You can't link to that.");
    return NOTHING;
  } else {
    return room;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * open_exit, do_open: Open a new exit and optionally link it somewhere.
 */

static void open_exit(EvaluationContext *evaluation, DbRef player, DbRef loc,
                      char *direction, char *linkto) {
  DbRef exit;
  LuaLockInvocation lock;
  LuaLockResult result;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  if (!direction || !*direction) {
    notify_quiet(evaluation, player, "Open where?");
    return;
  } else if (!is_controls(evaluation->world->database, player, loc)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  exit = create_obj(evaluation, player, TYPE_EXIT, direction);
  if (exit == NOTHING)
    return;

  /*
   * Initialize everything and link it in.
   */

  game_object_set_exits(evaluation->world->database, exit, loc);
  game_object_set_next(evaluation->world->database, exit,
                       game_object_exits(evaluation->world->database, loc));
  game_object_set_exits(evaluation->world->database, loc, exit);

  /*
   * and we're done
   */

  notify_quiet(evaluation, player, "Opened.");

  /*
   * See if we should do a link
   */

  if (!linkto || !*linkto)
    return;

  loc = parse_linkable_room(evaluation, &evaluation->command->match, player,
                            linkto);
  if (loc != NOTHING) {

    /*
     * Make sure the player passes the link lock
     */

    if (!lock_test(evaluation, player, player, player, loc, LUA_LOCK_LINK,
                   LUA_LOCK_OPERATION_LINK, false, &lock, &result)) {
      notify_lock_failure(evaluation, &lock, &result,
                          "You can't link to there.", nullptr, LUA_EVENT_NONE);
      return;
    }
    game_object_set_location(evaluation->world->database, exit, loc);
    notify_quiet(evaluation, player, "Linked.");
  }
}

void do_open(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *direction = invocation->first;
  char **links = invocation->vector;
  int nlinks = invocation->vector_count;
  DbRef loc, destnum;
  char *dest;

  /*
   * Create the exit and link to the destination, if there is one
   */

  if (nlinks >= 1)
    dest = links[0];
  else
    dest = nullptr;

  if (key == OPEN_INVENTORY)
    loc = player;
  else
    loc = game_object_location(evaluation->world->database, player);

  open_exit(evaluation, player, loc, direction, dest);

  /*
   * Open the back link if we can
   */

  if (nlinks >= 2) {
    destnum = parse_linkable_room(evaluation, &invocation->context->match,
                                  player, dest);
    if (destnum != NOTHING) {
      open_exit(evaluation, player, destnum, links[1], tprintf("%ld", loc));
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * link_exit, do_link: Set destination(exits), dropto(rooms) or
 * * home(player,thing)
 */

static void link_exit(EvaluationContext *evaluation, DbRef player, DbRef exit,
                      DbRef dest) {
  LuaLockInvocation lock;
  LuaLockResult result;

  /*
   * Make sure we can link there
   */

  if (dest != HOME) {
    if (!is_controls(evaluation->world->database, player, dest)) {
      notify_quiet(evaluation, player, "Permission denied.");
      return;
    }
    if (!lock_test(evaluation, player, player, player, dest, LUA_LOCK_LINK,
                   LUA_LOCK_OPERATION_LINK, false, &lock, &result)) {
      notify_lock_failure(evaluation, &lock, &result, "Permission denied.",
                          nullptr, LUA_EVENT_NONE);
      return;
    }
  }
  /*
   * Exit must be unlinked or controlled by you
   */

  if ((game_object_location(evaluation->world->database, exit) != NOTHING) &&
      !is_controls(evaluation->world->database, player, exit)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  /*
   * link has been validated and paid for, do it and tell the player
   */

  game_object_set_location(evaluation->world->database, exit, dest);
  if (!is_quiet(evaluation->world->database, player))
    notify_quiet(evaluation, player, "Linked.");
}

void do_link(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *what = invocation->first;
  char *where = invocation->second;
  DbRef thing, room;
  LuaLockInvocation lock;
  LuaLockResult result;

  /*
   * Find the thing to link
   */

  init_match(&invocation->context->match, player, what, TYPE_EXIT);
  match_everything(&invocation->context->match, 0);
  thing = noisy_match_result(&invocation->context->match);
  if (thing == NOTHING)
    return;

  /*
   * Allow unlink if where is not specified
   */

  if (!where || !*where) {
    CommandInvocation unlink_invocation = *invocation;

    unlink_invocation.first = what;
    do_unlink(&unlink_invocation);
    return;
  }
  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_EXIT:

    /*
     * Set destination
     */

    room = parse_linkable_room(evaluation, &invocation->context->match, player,
                               where);
    if (room != NOTHING)
      link_exit(evaluation, player, thing, room);
    break;
  case TYPE_PLAYER:
  case TYPE_THING:

    /*
     * Set home
     */

    if (!is_controls(evaluation->world->database, player, thing)) {
      notify_quiet(evaluation, player, "Permission denied.");
      break;
    }
    init_match(&invocation->context->match, player, where, NOTYPE);
    match_everything(&invocation->context->match, MAT_NO_EXITS);
    room = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, room))
      break;
    if (!has_contents(evaluation->world->database, room)) {
      notify_quiet(evaluation, player, "Can't link to an exit.");
      break;
    }
    if (!can_set_home(evaluation, player, thing, room)) {
      notify_quiet(evaluation, player, "Permission denied.");
    } else if (!lock_test(evaluation, player, invocation->cause, player, room,
                          LUA_LOCK_LINK, LUA_LOCK_OPERATION_SET_HOME, false,
                          &lock, &result)) {
      notify_lock_failure(evaluation, &lock, &result, "Permission denied.",
                          nullptr, LUA_EVENT_NONE);
    } else if (room == HOME) {
      notify_quiet(evaluation, player, "Can't set home to home.");
    } else {
      game_object_set_link(evaluation->world->database, thing, room);
      if (!is_quiet(evaluation->world->database, player))
        notify_quiet(evaluation, player, "Home set.");
    }
    break;
  case TYPE_ROOM:

    /*
     * Set dropto
     */

    if (!is_controls(evaluation->world->database, player, thing)) {
      notify_quiet(evaluation, player, "Permission denied.");
      break;
    }
    room = parse_linkable_room(evaluation, &invocation->context->match, player,
                               where);
    if (!(is_good_obj(evaluation->world->database, room) || (room == HOME)))
      break;

    if ((room != HOME) && !is_room(evaluation->world->database, room)) {
      notify_quiet(evaluation, player, "That is not a room!");
    } else if ((room != HOME) &&
               !is_controls(evaluation->world->database, player, room)) {
      notify_quiet(evaluation, player, "Permission denied.");
    } else if ((room != HOME) &&
               !lock_test(evaluation, player, invocation->cause, player, room,
                          LUA_LOCK_LINK, LUA_LOCK_OPERATION_LINK, false, &lock,
                          &result)) {
      notify_lock_failure(evaluation, &lock, &result, "Permission denied.",
                          nullptr, LUA_EVENT_NONE);
    } else {
      game_object_set_location(evaluation->world->database, thing, room);
      if (!is_quiet(evaluation->world->database, player))
        notify_quiet(evaluation, player, "Dropto set.");
    }
    break;
  case TYPE_GARBAGE:
    notify_quiet(evaluation, player, "Permission denied.");
    break;
  default:
    log_error(evaluation->log, LOG_BUGS, "BUG", "OTYPE",
              "Strange object type: object #%ld = %d", thing,
              typeof_obj(evaluation->world->database, thing));
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_dig: Create a new room.
 */

void do_dig(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  int key = invocation->key;
  char *name = invocation->first;
  char **args = invocation->vector;
  int nargs = invocation->vector_count;
  DbRef room;
  char *buff;

  /*
   * we don't need to know player's location!  hooray!
   */

  if (!name || !*name) {
    notify_quiet(evaluation, player, "Dig what?");
    return;
  }
  room = create_obj(evaluation, player, TYPE_ROOM, name);
  if (room == NOTHING)
    return;

  notify_printf(evaluation, player, "%s created with room number %ld.", name,
                room);

  buff = alloc_sbuf("do_dig");
  if ((nargs >= 1) && args[0] && *args[0]) {
    snprintf(buff, SBUF_SIZE, "%ld", room);
    open_exit(evaluation, player,
              game_object_location(evaluation->world->database, player),
              args[0], buff);
  }
  if ((nargs >= 2) && args[1] && *args[1]) {
    snprintf(buff, SBUF_SIZE, "%ld",
             game_object_location(evaluation->world->database, player));
    open_exit(evaluation, player, room, args[1], buff);
  }
  free_sbuf(buff);
  if (key == DIG_TELEPORT)
    (void)move_via_teleport(evaluation, player, room, cause, 0);
}

/*
 * ---------------------------------------------------------------------------
 * * do_create: Make a new object.
 */

void do_create(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *coststr = invocation->second;
  DbRef thing;
  char clearbuffer[MBUF_SIZE];

  (void)coststr;
  strip_ansi_r(clearbuffer, name, MBUF_SIZE);
  if (!name || !*name || (strlen(clearbuffer) == 0)) {
    notify_quiet(evaluation, player, "Create what?");
    return;
  }
  thing = create_obj(evaluation, player, TYPE_THING, name);
  if (thing == NOTHING)
    return;

  move_via_generic(evaluation, thing, player, NOTHING, 0);
  game_object_set_link(evaluation->world->database, thing,
                       new_home(evaluation, player));
  if (!is_quiet(evaluation->world->database, player)) {
    notify_printf(evaluation, player, "%s created as object #%ld",
                  game_object_name(invocation->context->world->database, thing),
                  thing);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_clone: Create a copy of an object.
 */

void do_clone(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  char *arg2 = invocation->second;
  DbRef clone, thing, loc;
  Flag rmv_flags;

  if ((key & CLONE_INVENTORY) ||
      !has_location(evaluation->world->database, player))
    loc = player;
  else
    loc = game_object_location(evaluation->world->database, player);

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  init_match(&invocation->context->match, player, name, NOTYPE);
  match_everything(&invocation->context->match, 0);
  thing = noisy_match_result(&invocation->context->match);
  if ((thing == NOTHING) || (thing == AMBIGUOUS))
    return;

  /* Cloning requires examination permission. */

  if (!is_examinable(evaluation->world->database, player, thing)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  if (is_player(evaluation->world->database, thing)) {
    notify_quiet(evaluation, player, "You cannot clone players!");
    return;
  }
  if ((typeof_obj(evaluation->world->database, thing) == TYPE_EXIT) &&
      !is_controls(evaluation->world->database, player, loc)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }

  /*
   * Go make the clone object
   */

  if ((arg2 && *arg2) &&
      ok_name(invocation->context->world->configuration, arg2))
    clone = create_obj(evaluation, player,
                       typeof_obj(evaluation->world->database, thing), arg2);
  else
    clone = create_obj(
        evaluation, player, typeof_obj(evaluation->world->database, thing),
        game_object_name(invocation->context->world->database, thing));
  if (clone == NOTHING)
    return;

  /*
   * Wipe out any old attributes and copy in the new data
   */

  attribute_free(evaluation->world->database, clone);
  attribute_copy(evaluation, player, clone, thing);

  /*
   * Reset the name, since we cleared the attributes
   */

  if ((arg2 && *arg2) &&
      ok_name(invocation->context->world->configuration, arg2))
    object_name_set(invocation->context->world->database, clone, arg2);
  else
    object_name_set(
        invocation->context->world->database, clone,
        game_object_name(invocation->context->world->database, thing));

  /*
   * Clear out problem flags from the original
   */

  rmv_flags = WIZARD;
  game_object_set_flags(evaluation->world->database, clone,
                        game_object_flags(evaluation->world->database, thing) &
                            ~rmv_flags);

  /*
   * Tell creator about it
   */

  if (!is_quiet(evaluation->world->database, player)) {
    if (arg2 && *arg2)
      notify_printf(
          evaluation, player, "%s cloned as %s, new copy is object #%ld.",
          game_object_name(invocation->context->world->database, thing), arg2,
          clone);
    else
      notify_printf(
          evaluation, player, "%s cloned, new copy is object #%ld.",
          game_object_name(invocation->context->world->database, thing), clone);
  }
  /*
   * Put the new thing in its new home.  Break any dropto or link, then
   * * * * * * * try to re-establish it.
   */

  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_THING:
    game_object_set_link(evaluation->world->database, clone,
                         clone_home(evaluation, player, thing));
    move_via_generic(evaluation, clone, loc, player, 0);
    break;
  case TYPE_ROOM:
    game_object_set_location(evaluation->world->database, clone, NOTHING);
    if (game_object_location(evaluation->world->database, thing) != NOTHING)
      link_exit(evaluation, player, clone,
                game_object_location(evaluation->world->database, thing));
    break;
  case TYPE_EXIT:
    game_object_set_exits(
        evaluation->world->database, loc,
        insert_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, loc),
                     clone));
    game_object_set_exits(evaluation->world->database, clone, loc);
    game_object_set_location(evaluation->world->database, clone, NOTHING);
    if (game_object_location(evaluation->world->database, thing) != NOTHING)
      link_exit(evaluation, player, clone,
                game_object_location(evaluation->world->database, thing));
    break;
  default:
    break;
  }

  notify_event(evaluation, invocation->context->descriptor, player,
               invocation->cause, clone, LUA_EVENT_CLONE, (char **)nullptr, 0);
}

/*
 * ---------------------------------------------------------------------------
 * * do_pcreate: Create new players.
 */

void do_pcreate(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *pass = invocation->second;
  DbRef newplayer;

  newplayer = create_player(evaluation, name, pass);
  if (newplayer == NOTHING) {
    notify_quiet(evaluation, player, tprintf("Failure creating '%s'", name));
    return;
  }
  move_object(evaluation, newplayer,
              invocation->context->world->configuration->start_room);
  notify_quiet(evaluation, player,
               tprintf("New player '%s' (#%ld) created with password '%s'",
                       name, newplayer, pass));

  STARTLOG(evaluation->log, LOG_PCREATES | LOG_WIZARD, "WIZ", "PCREA") {
    log_name(evaluation->log, newplayer);
    log_text(" created by ");
    log_name(evaluation->log, player);
    ENDLOG(evaluation->log);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * can_destroy_exit, can_destroy_player, do_destroy:
 * * Destroy things.
 */

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
    init_match(&invocation->context->match, player, what, TYPE_EXIT);
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
  case TYPE_EXIT:
    if (can_destroy_exit(evaluation, player, thing)) {
      if (is_going(evaluation->world->database, thing)) {
        notify_quiet(evaluation, player, "No sense beating a dead exit.");
      } else {
        if (is_hardcode(evaluation->world->database, thing)) {
          DisposeSpecialObject(evaluation->btech, player, thing);
          c_hardcode(evaluation->world->database, thing);
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
  case TYPE_THING:
    if (is_going(evaluation->world->database, thing)) {
      notify_quiet(evaluation, player, "No sense beating a dead object.");
    } else {
      if (is_hardcode(evaluation->world->database, thing)) {
        DisposeSpecialObject(evaluation->btech, player, thing);
        c_hardcode(evaluation->world->database, thing);
      }
      if (0) {
        destroy_thing(evaluation, thing);
      } else {
        notify(evaluation, player, "The object shakes and begins to crumble.");
        s_going(evaluation->world->database, thing);
      }
    }
    break;
  case TYPE_PLAYER:
    if (can_destroy_player(evaluation, player, thing)) {
      if (is_going(evaluation->world->database, thing)) {
        notify_quiet(evaluation, player, "No sense beating a dead player.");
      } else {
        if (is_hardcode(evaluation->world->database, thing)) {
          DisposeSpecialObject(evaluation->btech, player, thing);
          c_hardcode(evaluation->world->database, thing);
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
  case TYPE_ROOM:
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
void do_chzone(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *newobj = invocation->second;
  DbRef thing;
  DbRef zone;

  if (!invocation->context->world->configuration->have_zones) {
    notify(evaluation, player, "Zones disabled.");
    return;
  }
  init_match(&invocation->context->match, player, name, NOTYPE);
  match_everything(&invocation->context->match, 0);
  if ((thing = noisy_match_result(&invocation->context->match)) == NOTHING)
    return;

  if (!strcasecmp(newobj, "none"))
    zone = NOTHING;
  else {
    init_match(&invocation->context->match, player, newobj, NOTYPE);
    match_everything(&invocation->context->match, 0);
    if ((zone = noisy_match_result(&invocation->context->match)) == NOTHING)
      return;

    if ((typeof_obj(evaluation->world->database, zone) != TYPE_THING) &&
        (typeof_obj(evaluation->world->database, zone) != TYPE_ROOM)) {
      notify(evaluation, player, "Invalid zone object type.");
      return;
    }
  }

  if (!is_controls(evaluation->world->database, player, thing)) {
    notify(evaluation, player, "You don't have the power to shift reality.");
    return;
  }
  /* The target zone must also be controllable by the actor. */
  if ((zone != NOTHING) &&
      !is_controls(evaluation->world->database, player, zone)) {
    notify(evaluation, player, "You cannot move that object to that zone.");
    return;
  }
  /*
   * only rooms may be zoned to other rooms
   */
  if ((zone != NOTHING) &&
      (typeof_obj(evaluation->world->database, zone) == TYPE_ROOM) &&
      typeof_obj(evaluation->world->database, thing) != TYPE_ROOM) {
    notify(evaluation, player, "Only rooms may be zoned to rooms.");
    return;
  }
  /*
   * everything is okay, do the change
   */
  game_object_set_zone(invocation->context->world->database, thing, zone);
  if (typeof_obj(evaluation->world->database, thing) != TYPE_PLAYER) {
    /*
     * if the object is a player, resetting these flags is rather
     * * * * * inconvenient -- although this may pose a bit of a *
     * *  * security * risk. Be careful when @chzone'ing wizard players.
     */
    game_object_set_flags(
        evaluation->world->database, thing,
        game_object_flags(evaluation->world->database, thing) & ~WIZARD);
#ifdef USE_POWERS
    game_object_set_powers(evaluation->world->database, thing,
                           0); /*
                                * wipe out all powers
                                */
#endif
  }
  notify(evaluation, player, "Zone changed.");
}
void do_name(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *newname = invocation->second;
  DbRef thing;
  char *buff;
  char new[LBUF_SIZE];

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;

  /*
   * check for bad name
   */
  strncpy(new, newname, LBUF_SIZE - 1);
  if ((*newname == '\0') ||
      (strlen(strip_ansi_r(new, newname, strlen(newname))) == 0)) {
    notify_quiet(evaluation, player, "Give it what new name?");
    return;
  }
  /*
   * check for renaming a player
   */
  if (is_player(evaluation->world->database, thing)) {

    buff = trim_spaces((char *)newname);
    if (!ok_player_name(invocation->context->world->configuration, buff) ||
        !badname_check(invocation->context->world, buff)) {
      notify_quiet(evaluation, player, "You can't use that name.");
      free_lbuf(buff);
      return;
    } else if (string_compare(
                   invocation->context->world->configuration, buff,
                   game_object_name(invocation->context->world->database,
                                    thing)) &&
               (lookup_player(invocation->context->world, NOTHING, buff, 0) !=
                NOTHING)) {

      /*
       * string_compare allows changing foo to Foo, etc.
       */

      notify_quiet(evaluation, player, "That name is already in use.");
      free_lbuf(buff);
      return;
    }

    /*
     * everything ok, notify
     */
    STARTLOG(evaluation->log, LOG_SECURITY, "SEC", "CNAME") {
      log_name(evaluation->log, thing), log_text(" renamed to ");
      log_text(buff);
      ENDLOG(evaluation->log);
    }
    if (is_suspect(evaluation->world->database, thing)) {
      send_channel(
          evaluation, "Suspect", "%s",
          tprintf("%s renamed to %s",
                  game_object_name(invocation->context->world->database, thing),
                  buff));
    }
    delete_player_name(
        invocation->context->world, thing,
        game_object_name(invocation->context->world->database, thing));

    object_name_set(invocation->context->world->database, thing, buff);
    add_player_name(
        invocation->context->world, thing,
        game_object_name(invocation->context->world->database, thing));
    if (!is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_quiet(evaluation, player, "Name set.");
    free_lbuf(buff);
    return;
  } else {
    if (!ok_name(invocation->context->world->configuration, newname)) {
      notify_quiet(evaluation, player, "That is not a reasonable name.");
      return;
    }
    /*
     * everything ok, change the name
     */
    object_name_set(invocation->context->world->database, thing, newname);
    if (!is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_quiet(evaluation, player, "Name set.");
  }
}
/*
 * ---------------------------------------------------------------------------
 * * do_unlink: Unlink an exit from its destination or remove a dropto.
 */

void do_unlink(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  DbRef exit;

  init_match(&invocation->context->match, player, name, TYPE_EXIT);
  match_everything(&invocation->context->match, 0);
  exit = match_result(&invocation->context->match);

  switch (exit) {
  case NOTHING:
    notify_quiet(evaluation, player, "Unlink what?");
    break;
  case AMBIGUOUS:
    notify_quiet(evaluation, player, "I don't know which one you mean!");
    break;
  default:
    if (!is_controls(evaluation->world->database, player, exit)) {
      notify_quiet(evaluation, player, "Permission denied.");
    } else {
      switch (typeof_obj(evaluation->world->database, exit)) {
      case TYPE_EXIT:
        game_object_set_location(evaluation->world->database, exit, NOTHING);
        if (!is_quiet(evaluation->world->database, player))
          notify_quiet(evaluation, player, "Unlinked.");
        break;
      case TYPE_ROOM:
        game_object_set_location(evaluation->world->database, exit, NOTHING);
        if (!is_quiet(evaluation->world->database, player))
          notify_quiet(evaluation, player, "Dropto removed.");
        break;
      default:
        notify_quiet(evaluation, player, "You can't unlink that!");
        break;
      }
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_set: Set flags or attributes on objects, or flags on attributes.
 */
void do_set(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  char *flag = invocation->second;
  char *separator;
  DbRef dynamic_thing;

  separator = strchr(name, '/');
  if (separator) {
    *separator++ = '\0';
    dynamic_thing = match_controlled(&invocation->context->match, player, name);
    if (dynamic_thing == NOTHING)
      return;
    if (!dynamic_attribute_set(evaluation->world->database, dynamic_thing,
                               separator, flag))
      notify_quiet(evaluation, player, "Attribute update failed.");
    return;
  }
  separator = flag ? strchr(flag, ':') : nullptr;
  if (separator) {
    *separator++ = '\0';
    dynamic_thing = match_controlled(&invocation->context->match, player, name);
    if (dynamic_thing == NOTHING)
      return;
    if (!dynamic_attribute_set(evaluation->world->database, dynamic_thing, flag,
                               separator))
      notify_quiet(evaluation, player, "Attribute update failed.");
    return;
  }
  dynamic_thing = match_controlled(&invocation->context->match, player, name);
  if (dynamic_thing != NOTHING)
    flag_set(evaluation, invocation->context->world->indexes, dynamic_thing,
             player, flag, key);
  return;
}
void do_cpattr(CommandInvocation *invocation) {
  char *oldpair = invocation->first;
  char **newpair = invocation->vector;
  int nargs = invocation->vector_count;
  char *source_name = oldpair;
  char *source_attribute = strchr(source_name, '/');

  if (!source_attribute || nargs < 1)
    return;
  *source_attribute++ = '\0';
  DbRef source = match_controlled(&invocation->context->match,
                                  invocation->player, source_name);
  if (source == NOTHING)
    return;
  const char *value = dynamic_attribute_get(
      invocation->context->world->database, source, source_attribute);
  if (!value)
    value = "";
  for (int index = 0; index < nargs; index++) {
    char *destination_name = newpair[index];
    char *destination_attribute = strchr(destination_name, '/');
    if (destination_attribute)
      *destination_attribute++ = '\0';
    else
      destination_attribute = source_attribute;
    DbRef destination = match_controlled(&invocation->context->match,
                                         invocation->player, destination_name);
    if (destination != NOTHING)
      dynamic_attribute_set(invocation->context->world->database, destination,
                            destination_attribute, value);
  }
  return;
}

void do_mvattr(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *what = invocation->first;
  char **args = invocation->vector;
  int nargs = invocation->vector_count;
  if (nargs < 2)
    return;
  DbRef dynamic_thing =
      match_controlled(&invocation->context->match, player, what);
  if (dynamic_thing == NOTHING)
    return;
  const char *dynamic_value = dynamic_attribute_get(evaluation->world->database,
                                                    dynamic_thing, args[0]);
  if (!dynamic_value)
    dynamic_value = "";
  bool retain_source = false;
  for (int index = 1; index < nargs; index++) {
    if (strcmp(args[index], args[0]) == 0)
      retain_source = true;
    if (!dynamic_attribute_set(evaluation->world->database, dynamic_thing,
                               args[index], dynamic_value))
      notify_printf(evaluation, player, "%s: invalid attribute name.",
                    args[index]);
  }
  if (!retain_source)
    dynamic_attribute_delete(evaluation->world->database, dynamic_thing,
                             args[0]);
  return;
}
void do_wipe(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *it = invocation->first;
  char *pattern = strchr(it, '/');
  if (!pattern) {
    notify_quiet(evaluation, player, "Use @wipe object/pattern.");
    return;
  }
  *pattern++ = '\0';
  DbRef dynamic_thing =
      match_controlled(&invocation->context->match, player, it);
  if (dynamic_thing == NOTHING)
    return;
  GameObject *object =
      game_database_object(evaluation->world->database, dynamic_thing);
  int removed = 0;
  for (int index = object->at_count - 1; index >= 0; index--) {
    if (quick_wild(pattern, object->ahead[index].name)) {
      dynamic_attribute_delete(evaluation->world->database, dynamic_thing,
                               object->ahead[index].name);
      removed++;
    }
  }
  notify_printf(evaluation, player, "%d attribute%s cleared.", removed,
                removed == 1 ? "" : "s");
  return;
}
