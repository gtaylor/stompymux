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
             !is_linkable(evaluation, player, room)) {
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
  } else if (!is_controls(evaluation, player, loc)) {
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
    if (!is_controls(evaluation, player, dest)) {
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
      !is_controls(evaluation, player, exit)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  if (game_object_owner(evaluation->world->database, exit) !=
      game_object_owner(evaluation->world->database, player)) {
    game_object_set_owner(
        evaluation->world->database, exit,
        game_object_owner(evaluation->world->database, player));
    game_object_set_flags(
        evaluation->world->database, exit,
        (game_object_flags(evaluation->world->database, exit) &
         ~(INHERIT | WIZARD)) |
            HALT);
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

    if (!is_controls(evaluation, player, thing)) {
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

    if (!is_controls(evaluation, player, thing)) {
      notify_quiet(evaluation, player, "Permission denied.");
      break;
    }
    room = parse_linkable_room(evaluation, &invocation->context->match, player,
                               where);
    if (!(is_good_obj(evaluation->world->database, room) || (room == HOME)))
      break;

    if ((room != HOME) && !is_room(evaluation->world->database, room)) {
      notify_quiet(evaluation, player, "That is not a room!");
    } else if ((room != HOME) && !is_controls(evaluation, player, room)) {
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
 * * do_parent: Set an object's parent field.
 */

void do_parent(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *tname = invocation->first;
  char *pname = invocation->second;
  DbRef thing, parent, curr;
  int lev;

  /*
   * get victim
   */

  init_match(&invocation->context->match, player, tname, NOTYPE);
  match_everything(&invocation->context->match, 0);
  thing = noisy_match_result(&invocation->context->match);
  if (thing == NOTHING)
    return;

  /*
   * Make sure we can do it
   */

  if (!is_controls(evaluation, player, thing)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  /*
   * Find out what the new parent is
   */

  if (*pname) {
    init_match(&invocation->context->match, player, pname,
               typeof_obj(evaluation->world->database, thing));
    match_everything(&invocation->context->match, 0);
    parent = noisy_match_result(&invocation->context->match);
    if (parent == NOTHING)
      return;

    /*
     * Make sure we have rights to set parent
     */

    if (!is_parentable(evaluation, player, parent)) {
      notify_quiet(evaluation, player, "Permission denied.");
      return;
    }
    /*
     * Verify no recursive reference
     */

    ITER_PARENTS(evaluation->world->database,
                 invocation->context->world->configuration, parent, curr, lev) {
      if (curr == thing) {
        notify_quiet(evaluation, player,
                     "You can't have yourself as a parent!");
        return;
      }
    }
  } else {
    parent = NOTHING;
  }

  game_object_set_parent(evaluation->world->database, thing, parent);
  if (!is_quiet(evaluation->world->database, thing) &&
      !is_quiet(evaluation->world->database, player)) {
    if (parent == NOTHING)
      notify_quiet(evaluation, player, "Parent cleared.");
    else
      notify_quiet(evaluation, player, "Parent set.");
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
  DbRef clone, thing, new_owner, loc;
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

  if (!is_examinable(evaluation, player, thing)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }
  if (is_player(evaluation->world->database, thing)) {
    notify_quiet(evaluation, player, "You cannot clone players!");
    return;
  }
  /*
   * You can only make a parent link to what you control
   */

  if (!is_controls(evaluation, player, thing) && (key & CLONE_PARENT)) {
    notify_quiet(
        evaluation, player,
        tprintf("You don't control %s, ignoring /parent.",
                game_object_name(invocation->context->world->database, thing)));
    key &= ~CLONE_PARENT;
  }
  new_owner = (key & CLONE_PRESERVE)
                  ? game_object_owner(evaluation->world->database, thing)
                  : game_object_owner(evaluation->world->database, player);
  if ((typeof_obj(evaluation->world->database, thing) == TYPE_EXIT) &&
      !is_controls(evaluation, player, loc)) {
    notify_quiet(evaluation, player, "Permission denied.");
    return;
  }

  /*
   * Go make the clone object
   */

  if ((arg2 && *arg2) &&
      ok_name(invocation->context->world->configuration, arg2))
    clone = create_obj(evaluation, new_owner,
                       typeof_obj(evaluation->world->database, thing), arg2);
  else
    clone = create_obj(
        evaluation, new_owner, typeof_obj(evaluation->world->database, thing),
        game_object_name(invocation->context->world->database, thing));
  if (clone == NOTHING)
    return;

  /*
   * Wipe out any old attributes and copy in the new data
   */

  attribute_free(evaluation->world->database, clone);
  if (key & CLONE_PARENT)
    game_object_set_parent(evaluation->world->database, clone, thing);
  else
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
  if (!(key & CLONE_INHERIT) ||
      (!is_inherits(evaluation->world->database, player)))
    rmv_flags |= INHERIT;
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

  /*
   * If same owner run ACLONE, else halt it.  Also copy parent * if we
   * * * * * * can
   */

  if (new_owner == game_object_owner(evaluation->world->database, thing)) {
    if (!(key & CLONE_PARENT))
      game_object_set_parent(
          evaluation->world->database, clone,
          game_object_parent(evaluation->world->database, thing));
    notify_event(evaluation, invocation->context->descriptor, player,
                 invocation->cause, clone, LUA_EVENT_CLONE, (char **)nullptr,
                 0);
  } else {
    if (!(key & CLONE_PARENT) && is_controls(evaluation, player, thing))
      game_object_set_parent(
          evaluation->world->database, clone,
          game_object_parent(evaluation->world->database, thing));
    s_halted(evaluation->world->database, clone);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_pcreate: Create new players and robots.
 */

void do_pcreate(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  char *pass = invocation->second;
  int isrobot;
  DbRef newplayer;

  isrobot = (key == PCRE_ROBOT) ? 1 : 0;
  newplayer = create_player(evaluation, name, pass, player, isrobot);
  if (newplayer == NOTHING) {
    notify_quiet(evaluation, player, tprintf("Failure creating '%s'", name));
    return;
  }
  if (isrobot) {
    move_object(evaluation, newplayer,
                game_object_location(evaluation->world->database, player));
    notify_quiet(evaluation, player,
                 tprintf("New robot '%s' (#%ld) created with password '%s'",
                         name, newplayer, pass));

    notify_quiet(evaluation, player, "Your robot has arrived.");
    STARTLOG(evaluation->log, LOG_PCREATES, "CRE", "ROBOT") {
      log_name(evaluation->log, newplayer);
      log_text(" created by ");
      log_name(evaluation->log, player);
      ENDLOG(evaluation->log);
    }
  } else {
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
      (victim == configuration->start_room) ||
      (victim == configuration->master_room) || (victim == (DbRef)0) ||
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
      is_controls(evaluation, player,
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
          if (!is_quiet(evaluation->world->database, thing) &&
              !is_quiet(evaluation->world->database,
                        game_object_owner(evaluation->world->database, thing)))
            notify_quiet(
                evaluation,
                game_object_owner(evaluation->world->database, thing),
                tprintf("You will be rewarded shortly for %s(#%ld).",
                        game_object_name(invocation->context->world->database,
                                         thing),
                        thing));
          if ((game_object_owner(evaluation->world->database, thing) !=
               player) &&
              !is_quiet(evaluation->world->database, player))
            notify_quiet(
                evaluation, player,
                tprintf("Destroyed. #%ld's %s(#%ld)",
                        game_object_owner(evaluation->world->database, thing),
                        game_object_name(invocation->context->world->database,
                                         thing),
                        thing));
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
        if (!is_quiet(evaluation->world->database, thing) &&
            !is_quiet(evaluation->world->database,
                      game_object_owner(evaluation->world->database, thing)))
          notify_quiet(evaluation,
                       game_object_owner(evaluation->world->database, thing),
                       tprintf("You will be rewarded shortly for %s(#%ld).",
                               game_object_name(
                                   invocation->context->world->database, thing),
                               thing));
        if ((game_object_owner(evaluation->world->database, thing) != player) &&
            !is_quiet(evaluation->world->database, player))
          notify_quiet(
              evaluation, player,
              tprintf(
                  "Destroyed. %s's %s(#%ld)",
                  game_object_name(
                      invocation->context->world->database,
                      game_object_owner(evaluation->world->database, thing)),
                  game_object_name(invocation->context->world->database, thing),
                  thing));
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
        if (!is_quiet(evaluation->world->database, thing) &&
            !is_quiet(evaluation->world->database,
                      game_object_owner(evaluation->world->database, thing)))
          notify_quiet(evaluation,
                       game_object_owner(evaluation->world->database, thing),
                       tprintf("You will be rewarded shortly for %s(#%ld).",
                               game_object_name(
                                   invocation->context->world->database, thing),
                               thing));
        if ((game_object_owner(evaluation->world->database, thing) != player) &&
            !is_quiet(evaluation->world->database, player))
          notify_quiet(
              evaluation, player,
              tprintf(
                  "Destroyed. %s's %s(#%ld)",
                  game_object_name(
                      invocation->context->world->database,
                      game_object_owner(evaluation->world->database, thing)),
                  game_object_name(invocation->context->world->database, thing),
                  thing));
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

  if (!is_wizard(evaluation->world->database, player) &&
      !(is_controls(evaluation, player, thing)) &&
      !(check_zone_for_player(evaluation, player, thing)) &&
      !(game_object_owner(invocation->context->world->database, player) ==
        game_object_owner(invocation->context->world->database, thing))) {
    notify(evaluation, player, "You don't have the power to shift reality.");
    return;
  }
  /*
   * a player may change an object's zone to NOTHING or to an object he
   *
   * *  * *  * * owns
   */
  if ((zone != NOTHING) && !is_wizard(evaluation->world->database, player) &&
      !(is_controls(evaluation, player, zone)) &&
      !(game_object_owner(invocation->context->world->database, player) ==
        game_object_owner(invocation->context->world->database, zone))) {
    notify(evaluation, player, "You cannot move that object to that zone.");
    return;
  }
  /*
   * only rooms may be zoned to other rooms
   */
  if ((zone != NOTHING) &&
      (typeof_obj(evaluation->world->database, zone) == TYPE_ROOM) &&
      typeof_obj(evaluation->world->database, thing) != TYPE_ROOM) {
    notify(evaluation, player, "Only rooms may have parent rooms.");
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
    game_object_set_flags(
        evaluation->world->database, thing,
        game_object_flags(evaluation->world->database, thing) & ~INHERIT);
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
    if (!is_controls(evaluation, player, exit)) {
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
 * * do_chown: Change ownership of an object or attribute.
 */

void do_chown(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *newown = invocation->second;
  DbRef thing, owner, aowner;
  int atr, do_it;
  long aflags;
  Attribute *ap;

  if (parse_attrib(&invocation->context->match, player, name, &thing, &atr)) {
    if (atr != NOTHING) {
      if (!*newown) {
        owner = game_object_owner(evaluation->world->database, thing);
      } else if (!(string_compare(invocation->context->world->configuration,
                                  newown, "me"))) {
        owner = game_object_owner(evaluation->world->database, player);
      } else {
        owner = lookup_player(invocation->context->world, player, newown, 1);
      }

      /*
       * You may chown an attr to yourself if you own the *
       *
       * *  * *  * * object and the attr is not locked. *
       * You * may * chown  * an attr to the owner of the
       * object * if * * you own * the attribute. * To do
       * anything * else you  * must be a  * wizard. * Only
       * #1 can * chown * attributes on #1.
       */

      if (!attribute_get_info(evaluation->world->database, thing, atr, &aowner,
                              &aflags)) {
        notify_quiet(evaluation, player, "Attribute not present on object.");
        return;
      }
      do_it = 0;
      if (owner == NOTHING) {
        notify_quiet(evaluation, player, "I couldn't find that player.");
        // Same message as the final else below, but this is a distinct
        // god-protection check, not the general fallback.
      } else if (is_god(evaluation->world->database, thing) &&
                 !is_god(evaluation->world->database,
                         player)) { // NOLINT(bugprone-branch-clone)
        notify_quiet(evaluation, player, "Permission denied.");
      } else if (is_wizard(evaluation->world->database, player)) {
        do_it = 1;
      } else if (owner ==
                 game_object_owner(evaluation->world->database, player)) {

        /*
         * chown to me: only if I own the obj and * *
         *
         * *  * * !locked
         */

        if (!is_controls(evaluation, player, thing)) {
          notify_quiet(evaluation, player, "Permission denied.");
        } else {
          do_it = 1;
        }
      } else if (owner ==
                 game_object_owner(evaluation->world->database, thing)) {

        /*
         * chown to obj owner: only if I own attr * *
         *
         * *  * * and !locked
         */

        if (game_object_owner(evaluation->world->database, player) != aowner) {
          notify_quiet(evaluation, player, "Permission denied.");
        } else {
          do_it = 1;
        }
      } else {
        notify_quiet(evaluation, player, "Permission denied.");
      }

      if (!do_it)
        return;

      ap = attribute_by_number(invocation->context->world->database, atr);
      if (!ap || !set_attr(evaluation, player, player, ap, aflags)) {
        notify_quiet(evaluation, player, "Permission denied.");
        return;
      }
      attribute_set_owner(evaluation->world->database, thing, atr, owner);
      if (!is_quiet(evaluation->world->database, player))
        notify_quiet(evaluation, player, "Attribute owner changed.");
      return;
    }
  }
  init_match(&invocation->context->match, player, name, TYPE_THING);
  match_possession(&invocation->context->match);
  match_here(&invocation->context->match);
  match_exit(&invocation->context->match);
  match_me(&invocation->context->match);
  if (is_wizard(evaluation->world->database, player)) {
    match_player(&invocation->context->match);
    match_absolute(&invocation->context->match);
  }
  switch (thing = match_result(&invocation->context->match)) {
  case NOTHING:
    notify_quiet(evaluation, player, "You don't have that!");
    return;
  case AMBIGUOUS:
    notify_quiet(evaluation, player, "I don't know which you mean!");
    return;
  default:
    break;
  }

  if (!*newown || !(string_compare(invocation->context->world->configuration,
                                   newown, "me"))) {
    owner = game_object_owner(evaluation->world->database, player);
  } else {
    owner = lookup_player(invocation->context->world, player, newown, 1);
  }

  if (owner == NOTHING) {
    notify_quiet(evaluation, player, "I couldn't find that player.");
  } else if (is_player(evaluation->world->database, thing) &&
             !is_god(evaluation->world->database, player)) {
    notify_quiet(evaluation, player, "Players always own themselves.");
  } else if (((!is_controls(evaluation, player, thing) &&
               !is_wizard(evaluation->world->database, player)) ||
              (is_thing(evaluation->world->database, thing) &&
               (game_object_location(evaluation->world->database, thing) !=
                player) &&
               !is_wizard(evaluation->world->database, player))) ||
             (!is_controls(evaluation, player, owner))) {
    notify_quiet(evaluation, player, "Permission denied.");
  } else {
    if (is_god(evaluation->world->database, player)) {
      game_object_set_owner(evaluation->world->database, thing, owner);
    } else {
      game_object_set_owner(
          evaluation->world->database, thing,
          game_object_owner(evaluation->world->database, owner));
    }
    attribute_change_owner(evaluation->world->database, thing);
    game_object_set_flags(
        evaluation->world->database, thing,
        (game_object_flags(evaluation->world->database, thing) & ~INHERIT) |
            HALT);
    game_object_set_powers(evaluation->world->database, thing, 0);
    game_object_set_powers2(evaluation->world->database, thing, 0);
    halt_que(invocation->context->runtime->commands, NOTHING, thing);
    if (!is_quiet(evaluation->world->database, player))
      notify_quiet(evaluation, player, "Owner changed.");
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
  DbRef thing, thing2, aowner;
  char *p, *buff;
  int atr, atr2, clear, flagvalue, could_hear, have_xcode;
  long aflags;
  Attribute *attr, *attr2;

  /*
   * See if we have the <obj>/<attr> form, which is how you set * * *
   * attribute * flags.
   */

  if (parse_attrib(&invocation->context->match, player, name, &thing, &atr)) {
    if (atr != NOTHING) {

      /*
       * You must specify a flag name
       */

      if (!flag || !*flag) {
        notify_quiet(evaluation, player, "I don't know what you want to set!");
        return;
      }
      /*
       * Check for clearing
       */

      clear = 0;
      if (*flag == NOT_TOKEN) {
        flag++;
        clear = 1;
      }
      /*
       * Make sure player specified a valid attribute flag
       */

      flagvalue = name_table_search(invocation->context->world->database,
                                    invocation->context->world->configuration,
                                    player, indiv_attraccess_nametab, flag);
      if (flagvalue < 0) {
        notify_quiet(evaluation, player, "You can't set that!");
        return;
      }
      /*
       * Make sure the object has the attribute present
       */

      if (!attribute_get_info(evaluation->world->database, thing, atr, &aowner,
                              &aflags)) {
        notify_quiet(evaluation, player, "Attribute not present on object.");
        return;
      }
      /*
       * Make sure we can write to the attribute
       */

      attr = attribute_by_number(invocation->context->world->database, atr);
      if (!attr || !set_attr(evaluation, player, thing, attr, aflags)) {
        notify_quiet(evaluation, player, "Permission denied.");
        return;
      }
      /*
       * Go do it
       */

      if (clear)
        aflags &= ~flagvalue;
      else
        aflags |= flagvalue;
      have_xcode = is_hardcode(evaluation->world->database, thing);
      attribute_set_flags(evaluation->world->database, thing, atr, aflags);

      /*
       * Tell the player about it.
       */

      if (invocation->context->world->configuration->have_specials)
        handle_xcode(invocation->context->btech, player, thing, have_xcode,
                     is_hardcode(evaluation->world->database, thing));
      if (!(key & SET_QUIET) &&
          !is_quiet(evaluation->world->database, player) &&
          !is_quiet(evaluation->world->database, thing)) {
        NameTable *nt;
        nt = name_table_find_entry(invocation->context->world->database,
                                   invocation->context->world->configuration,
                                   player, indiv_attraccess_nametab, flag);
        notify_printf(
            evaluation, player, "%s/%s - %s %s",
            game_object_name(invocation->context->world->database, thing),
            attr->name, nt->name, clear ? "cleared." : "set.");
      }
      could_hear = is_hearer(evaluation, thing);
      handle_ears(evaluation, thing, could_hear, is_hearer(evaluation, thing));
      return;
    }
  }
  /*
   * find thing
   */

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;

  /*
   * check for attribute set first
   */
  for (p = flag; *p && (*p != ':'); p++)
    ;

  if (*p) {
    *p++ = 0;
    atr = mkattr(invocation->context->world->database, flag);
    if (atr <= 0) {
      notify_quiet(evaluation, player, "Couldn't create attribute.");
      return;
    }
    attr = attribute_by_number(invocation->context->world->database, atr);
    if (!attr) {
      notify_quiet(evaluation, player, "Permission denied.");
      return;
    }
    attribute_get_info(evaluation->world->database, thing, atr, &aowner,
                       &aflags);
    if (!set_attr(evaluation, player, thing, attr, aflags)) {
      notify_quiet(evaluation, player, "Permission denied.");
      return;
    }
    buff = alloc_lbuf("do_set");

    /*
     * check for _
     */
    if (*p == '_') {
      StringCopy(buff, p + 1);
      if (!parse_attrib(&invocation->context->match, player, p + 1, &thing2,
                        &atr2) ||
          (atr2 == NOTHING)) {
        notify_quiet(evaluation, player, "No match.");
        free_lbuf(buff);
        return;
      }
      attr2 = attribute_by_number(invocation->context->world->database, atr2);
      p = buff;
      attribute_parent_get_string(evaluation->world->database, buff, thing2,
                                  atr2, &aowner, &aflags);

      if (!attr2 ||
          !see_attr(evaluation, player, thing2, attr2, aowner, aflags)) {
        notify_quiet(evaluation, player, "Permission denied.");
        free_lbuf(buff);
        return;
      }
    }
    /*
     * Go set it
     */

    object_attribute_set(evaluation, invocation->context->world->configuration,
                         player, thing, atr, p, key);
    free_lbuf(buff);
    return;
  }
  /*
   * Set or clear a flag
   */

  flag_set(evaluation, invocation->context->world->indexes, thing, player, flag,
           key);
}
void do_cpattr(CommandInvocation *invocation) {
  char *oldpair = invocation->first;
  char **newpair = invocation->vector;
  int nargs = invocation->vector_count;
  int i;
  char *oldthing, *oldattr, *newthing, *newattr;

  if (!*oldpair || !**newpair || !oldpair || !*newpair)
    return;

  if (nargs < 1)
    return;

  oldattr = oldpair;
  oldthing =
      parse_to(invocation->context->world->configuration, &oldattr, '/', 1);

  for (i = 0; i < nargs; i++) {
    CommandInvocation set_invocation = *invocation;

    set_invocation.key = 0;
    newattr = newpair[i];
    newthing =
        parse_to(invocation->context->world->configuration, &newattr, '/', 1);

    if (!oldattr) {
      if (!newattr) {
        set_invocation.first = newthing;
        set_invocation.second = tprintf("%s:_%s/%s", oldthing, "me", oldthing);
        do_set(&set_invocation);
      } else {
        set_invocation.first = newthing;
        set_invocation.second = tprintf("%s:_%s/%s", newattr, "me", oldthing);
        do_set(&set_invocation);
      }
    } else {
      if (!newattr) {
        set_invocation.first = newthing;
        set_invocation.second =
            tprintf("%s:_%s/%s", oldattr, oldthing, oldattr);
        do_set(&set_invocation);
      } else {
        set_invocation.first = newthing;
        set_invocation.second =
            tprintf("%s:_%s/%s", newattr, oldthing, oldattr);
        do_set(&set_invocation);
      }
    }
  }
}

void do_mvattr(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *what = invocation->first;
  char **args = invocation->vector;
  int nargs = invocation->vector_count;
  DbRef thing, aowner, axowner;
  Attribute *in_attr, *out_attr;
  int i, anum, in_anum, no_delete;
  long aflags, axflags;
  char *astr;

  aflags = 0;

  /*
   * Make sure we have something to do.
   */

  if (nargs < 2) {
    notify_quiet(evaluation, player, "Nothing to do.");
    return;
  }
  /*
   * FInd and make sure we control the target object.
   */

  thing = match_controlled(&invocation->context->match, player, what);
  if (thing == NOTHING)
    return;

  /*
   * Look up the source attribute.  If it either doesn't exist or isn't
   * * * * * readable, use an empty string.
   */

  in_anum = -1;
  astr = alloc_lbuf("do_mvattr");
  in_attr = attribute_by_name(invocation->context->world->database, args[0]);
  if (in_attr == nullptr) {
    *astr = '\0';
  } else {
    attribute_get_string(evaluation->world->database, astr, thing,
                         in_attr->number, &aowner, &aflags);
    if (!see_attr(evaluation, player, thing, in_attr, aowner, aflags)) {
      *astr = '\0';
    } else {
      in_anum = in_attr->number;
    }
  }

  /*
   * Copy the attribute to each target in turn.
   */

  no_delete = 0;
  for (i = 1; i < nargs; i++) {
    anum = mkattr(invocation->context->world->database, args[i]);
    if (anum <= 0) {
      notify_quiet(
          evaluation, player,
          tprintf("%s: That's not a good name for an attribute.", args[i]));
      continue;
    }
    out_attr = attribute_by_number(invocation->context->world->database, anum);
    if (!out_attr) {
      notify_quiet(evaluation, player,
                   tprintf("%s: Permission denied.", args[i]));
    } else if (out_attr->number == in_anum) {
      no_delete = 1;
    } else {
      attribute_get_info(evaluation->world->database, thing, out_attr->number,
                         &axowner, &axflags);
      if (!set_attr(evaluation, player, thing, out_attr, axflags)) {
        notify_quiet(evaluation, player,
                     tprintf("%s: Permission denied.", args[i]));
      } else {
        attribute_add(
            evaluation->world->database, thing, out_attr->number, astr,
            game_object_owner(evaluation->world->database, player), aflags);
        if (!is_quiet(evaluation->world->database, player))
          notify_printf(
              evaluation, player, "%s/%s - Set.",
              game_object_name(invocation->context->world->database, thing),
              out_attr->name);
      }
    }
  }

  /*
   * Remove the source attribute if we can.
   */

  if ((in_anum > 0) && !no_delete) {
    in_attr =
        attribute_by_number(invocation->context->world->database, in_anum);
    if (in_attr && set_attr(evaluation, player, thing, in_attr, aflags)) {
      attribute_clear(evaluation->world->database, thing, in_attr->number);
      if (!is_quiet(evaluation->world->database, player))
        notify_printf(
            evaluation, player, "%s/%s - Cleared.",
            game_object_name(invocation->context->world->database, thing),
            in_attr->name);
    } else {
      if (in_attr)
        notify_quiet(
            evaluation, player,
            tprintf("%s: Could not remove old attribute.  Permission denied.",
                    in_attr->name));
    }
  }
  free_lbuf(astr);
}
void do_wipe(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *it = invocation->first;
  DbRef thing, aowner;
  int attr, got_one;
  long aflags;
  Attribute *ap;
  char *atext;
  ObjectList attributes;

  object_list_initialize(&attributes);
  if (!it || !*it ||
      !parse_attrib_wild(&invocation->context->match, player, it, &thing, 0, 0,
                         1, &attributes,
                         invocation->context->world->configuration,
                         invocation->context->runtime->world_indexes)) {
    notify_quiet(evaluation, player, "No match.");
    object_list_destroy(&attributes);
    return;
  }
  /*
   * Iterate through matching attributes, zapping the writable ones
   */

  got_one = 0;
  atext = alloc_lbuf("do_wipe.atext");
  for (attr = (int)object_list_first(&attributes); attr != NOTHING;
       attr = (int)object_list_next(&attributes)) {
    ap = attribute_by_number(invocation->context->world->database, attr);
    if (ap) {

      /*
       * Get the attr and make sure we can modify it.
       */

      attribute_get_string(evaluation->world->database, atext, thing,
                           ap->number, &aowner, &aflags);
      if (set_attr(evaluation, player, thing, ap, aflags)) {
        attribute_clear(evaluation->world->database, thing, ap->number);
        got_one++;
      }
    }
  }
  /*
   * Clean up
   */

  if (!got_one) {
    notify_quiet(evaluation, player, "No matching attributes.");
  } else {
    if (!is_quiet(evaluation->world->database, player))
      notify_printf(
          evaluation, player, "%s - %d attribute(s) wiped.",
          game_object_name(invocation->context->world->database, thing),
          got_one);
  }

  free_lbuf(atext);
  object_list_destroy(&attributes);
}
