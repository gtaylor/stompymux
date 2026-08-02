/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include "mux/commands/command.h"

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
#include "mux/support/styled_text.h"
#include "mux/world/match.h"
#include "mux/world/object.h"
#include "mux/world/object_set.h"
#include "mux/world/walkdb.h"

extern NameTable indiv_attraccess_nametab[];

static char *compile_object_name(EvaluationContext *evaluation, DbRef player,
                                 const char *name) {
  char *compiled = alloc_lbuf("compile_object_name");
  char error[256];

  if (styled_text_compile(evaluation->world->styled_text_palette, name,
                          compiled, LBUF_SIZE, error, sizeof(error))) {
    StringCopy(compiled, name);
    return compiled;
  }
  notify_printf(evaluation, player, "Invalid styled-text markup: %s.", error);
  free_lbuf(compiled);
  return nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * parse_linkable_room: Get a location to link to.
 */

static DbRef parse_linkable_room(EvaluationContext *evaluation,
                                 MatchContext *match, DbRef player,
                                 char *room_name) {
  DbRef room;

  init_match(match, player, room_name, OBJECT_TYPE_NOTYPE);
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
  char *compiled_direction;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  if (!direction || !*direction) {
    notify_quiet(evaluation, player, "Open where?");
    return;
  } else if (!is_controls(evaluation->world->database, player, loc)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  compiled_direction = compile_object_name(evaluation, player, direction);
  if (!compiled_direction)
    return;
  exit = create_obj(evaluation, player, OBJECT_TYPE_EXIT, compiled_direction);
  free_lbuf(compiled_direction);
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

  init_match(&invocation->context->match, player, what, OBJECT_TYPE_EXIT);
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
  case OBJECT_TYPE_EXIT:

    /*
     * Set destination
     */

    room = parse_linkable_room(evaluation, &invocation->context->match, player,
                               where);
    if (room != NOTHING)
      link_exit(evaluation, player, thing, room);
    break;
  case OBJECT_TYPE_PLAYER:
  case OBJECT_TYPE_THING:

    /*
     * Set home
     */

    if (!is_controls(evaluation->world->database, player, thing)) {
      notify_quiet(evaluation, player, "Permission denied.");
      break;
    }
    init_match(&invocation->context->match, player, where, OBJECT_TYPE_NOTYPE);
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
  case OBJECT_TYPE_ROOM:

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
  case OBJECT_TYPE_GARBAGE:
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
  char *compiled_name;

  /*
   * we don't need to know player's location!  hooray!
   */

  if (!name || !*name) {
    notify_quiet(evaluation, player, "Dig what?");
    return;
  }
  compiled_name = compile_object_name(evaluation, player, name);
  if (!compiled_name)
    return;
  room = create_obj(evaluation, player, OBJECT_TYPE_ROOM, compiled_name);
  free_lbuf(compiled_name);
  if (room == NOTHING)
    return;

  notify_printf(evaluation, player, "%s created with room number %ld.",
                game_object_name(evaluation->world->database, room), room);

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
  char *compiled_name;

  (void)coststr;
  compiled_name = compile_object_name(evaluation, player, name);
  if (!compiled_name)
    return;
  styled_text_strip(evaluation->world->styled_text_palette, compiled_name,
                    clearbuffer, MBUF_SIZE);
  if (!name || !*name || (strlen(clearbuffer) == 0)) {
    notify_quiet(evaluation, player, "Create what?");
    free_lbuf(compiled_name);
    return;
  }
  thing = create_obj(evaluation, player, OBJECT_TYPE_THING, compiled_name);
  free_lbuf(compiled_name);
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
  char *clone_name = nullptr;
  char pure_name[LBUF_SIZE];
  DbRef clone, thing, loc;
  Flag rmv_flags;

  if ((key & CLONE_INVENTORY) ||
      !has_location(evaluation->world->database, player))
    loc = player;
  else
    loc = game_object_location(evaluation->world->database, player);

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
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
  if ((typeof_obj(evaluation->world->database, thing) == OBJECT_TYPE_EXIT) &&
      !is_controls(evaluation->world->database, player, loc)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }

  /*
   * Go make the clone object
   */

  if (arg2 && *arg2) {
    clone_name = compile_object_name(evaluation, player, arg2);
    if (!clone_name)
      return;
    styled_text_strip(evaluation->world->styled_text_palette, clone_name,
                      pure_name, sizeof(pure_name));
    if (!ok_name(invocation->context->world->configuration, pure_name)) {
      notify_quiet(evaluation, player, "That is not a reasonable name.");
      free_lbuf(clone_name);
      return;
    }
  }
  if (clone_name)
    clone =
        create_obj(evaluation, player,
                   typeof_obj(evaluation->world->database, thing), clone_name);
  else
    clone = create_obj(
        evaluation, player, typeof_obj(evaluation->world->database, thing),
        game_object_name(invocation->context->world->database, thing));
  if (clone == NOTHING) {
    free_lbuf(clone_name);
    return;
  }

  /*
   * Wipe out any old attributes and copy in the new data
   */

  attribute_free(evaluation->world->database, clone);
  attribute_copy(evaluation, player, clone, thing);

  /*
   * Reset the name, since we cleared the attributes
   */

  if (clone_name)
    object_name_set(invocation->context->world->database, clone, clone_name);
  else
    object_name_set(
        invocation->context->world->database, clone,
        game_object_name(invocation->context->world->database, thing));
  free_lbuf(clone_name);

  /*
   * Clear out problem flags from the original
   */

  (void)rmv_flags;
  game_object_set_flag(evaluation->world->database, clone, OBJECT_FLAG_WIZARD,
                       false);

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
  case OBJECT_TYPE_THING:
    game_object_set_link(evaluation->world->database, clone,
                         clone_home(evaluation, player, thing));
    move_via_generic(evaluation, clone, loc, player, 0);
    break;
  case OBJECT_TYPE_ROOM:
    game_object_set_location(evaluation->world->database, clone, NOTHING);
    if (game_object_location(evaluation->world->database, thing) != NOTHING)
      link_exit(evaluation, player, clone,
                game_object_location(evaluation->world->database, thing));
    break;
  case OBJECT_TYPE_EXIT:
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
void do_chzone(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *newobj = invocation->second;
  DbRef thing;
  DbRef zone;

  init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
  match_everything(&invocation->context->match, 0);
  if ((thing = noisy_match_result(&invocation->context->match)) == NOTHING)
    return;

  if (!strcasecmp(newobj, "none"))
    zone = NOTHING;
  else {
    init_match(&invocation->context->match, player, newobj, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    if ((zone = noisy_match_result(&invocation->context->match)) == NOTHING)
      return;

    if ((typeof_obj(evaluation->world->database, zone) != OBJECT_TYPE_THING) &&
        (typeof_obj(evaluation->world->database, zone) != OBJECT_TYPE_ROOM)) {
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
      (typeof_obj(evaluation->world->database, zone) == OBJECT_TYPE_ROOM) &&
      typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_ROOM) {
    notify(evaluation, player, "Only rooms may be zoned to rooms.");
    return;
  }
  /*
   * everything is okay, do the change
   */
  game_object_set_zone(invocation->context->world->database, thing, zone);
  if (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_PLAYER) {
    /*
     * if the object is a player, resetting these flags is rather
     * * * * * inconvenient -- although this may pose a bit of a *
     * *  * security * risk. Be careful when @chzone'ing wizard players.
     */
    game_object_set_flag(evaluation->world->database, thing, OBJECT_FLAG_WIZARD,
                         false);
    game_object_clear_powers(evaluation->world->database, thing);
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
  char *compiled_name;

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;
  compiled_name = compile_object_name(evaluation, player, newname);
  if (!compiled_name)
    return;
  newname = compiled_name;

  /*
   * check for bad name
   */
  styled_text_strip(evaluation->world->styled_text_palette, newname, new,
                    sizeof(new));
  if (*newname == '\0' || strlen(new) == 0) {
    notify_quiet(evaluation, player, "Give it what new name?");
    free_lbuf(compiled_name);
    return;
  }
  /*
   * check for renaming a player
   */
  if (is_player(evaluation->world->database, thing)) {

    styled_text_strip(evaluation->world->styled_text_palette, newname, new,
                      sizeof(new));
    buff = trim_spaces(new);
    if (!ok_player_name(invocation->context->world->configuration, buff) ||
        !badname_check(invocation->context->world, buff)) {
      notify_quiet(evaluation, player, "You can't use that name.");
      free_lbuf(buff);
      free_lbuf(compiled_name);
      return;
    } else if (string_compare(
                   invocation->context->world->configuration, buff,
                   game_object_pure_name(invocation->context->world->database,
                                         thing)) &&
               (lookup_player(invocation->context->world, NOTHING, buff, 0) !=
                NOTHING)) {

      /*
       * string_compare allows changing foo to Foo, etc.
       */

      notify_quiet(evaluation, player, "That name is already in use.");
      free_lbuf(buff);
      free_lbuf(compiled_name);
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
        game_object_pure_name(invocation->context->world->database, thing));

    object_name_set(invocation->context->world->database, thing, newname);
    add_player_name(
        invocation->context->world, thing,
        game_object_pure_name(invocation->context->world->database, thing));
    if (!is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_quiet(evaluation, player, "Name set.");
    free_lbuf(buff);
    free_lbuf(compiled_name);
    return;
  } else {
    styled_text_strip(evaluation->world->styled_text_palette, newname, new,
                      sizeof(new));
    if (!ok_name(invocation->context->world->configuration, new)) {
      notify_quiet(evaluation, player, "That is not a reasonable name.");
      free_lbuf(compiled_name);
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
  free_lbuf(compiled_name);
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

  init_match(&invocation->context->match, player, name, OBJECT_TYPE_EXIT);
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
      case OBJECT_TYPE_EXIT:
        game_object_set_location(evaluation->world->database, exit, NOTHING);
        if (!is_quiet(evaluation->world->database, player))
          notify_quiet(evaluation, player, "Unlinked.");
        break;
      case OBJECT_TYPE_ROOM:
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
 * * do_set: Set flags on objects.
 */
void do_set(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  char *flag = invocation->second;
  DbRef thing = match_controlled(&invocation->context->match, player, name);
  if (thing != NOTHING)
    flag_set(evaluation, invocation->context->world->indexes, thing, player,
             flag, key);
}
