/*
 * builder_commands.c -- Commands that create and configure world objects
 */

#include <stdio.h>
#include <string.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/builder_commands_internal.h"
#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_invocation.h"
#include "mux/commands/command_keys.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/name_table.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/validation.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/object.h"
#include "mux/world/object_list.h"
#include "mux/world/object_set.h"
#include "mux/world/player.h"

extern NameTable indiv_attraccess_nametab[];

char *builder_compile_object_name(EvaluationContext *evaluation, DbRef player,
                                  const char *name) {
  char *compiled = alloc_lbuf("builder_compile_object_name");
  char error[256];

  if (styled_text_compile(evaluation->world->styled_text_palette, name,
                          compiled, LBUF_SIZE, error, sizeof(error))) {
    string_copy(compiled, name);
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
    notify_checked(evaluation, player, player, "That's not a valid object.",
                   MSG_ME);
    return NOTHING;
  } else if (!has_contents(evaluation->world->database, room) ||
             !is_linkable(evaluation->world->database, player, room)) {
    notify_checked(evaluation, player, player, "You can't link to that.",
                   MSG_ME);
    return NOTHING;
  } else {
    return room;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * open_exit, do_open: Open a new exit and optionally link it somewhere.
 */

typedef struct ExitCreationRequest {
  EvaluationContext *evaluation;
  DbRef player;
  DbRef location;
  char *direction;
  char *destination;
} ExitCreationRequest;

static void open_exit(const ExitCreationRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->player;
  DbRef loc = request->location;
  char *direction = request->direction;
  char *linkto = request->destination;
  DbRef exit;
  LuaLockInvocation lock;
  LuaLockResult result;
  char *compiled_direction;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  if (!direction || !*direction) {
    notify_checked(evaluation, player, player, "Open where?", MSG_ME);
    return;
  } else if (!is_controls(evaluation->world->database, player, loc)) {
    notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    return;
  }
  compiled_direction =
      builder_compile_object_name(evaluation, player, direction);
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

  notify_checked(evaluation, player, player, "Opened.", MSG_ME);

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
      notify_lock_failure(&(LockFailureNotification){
          .evaluation = evaluation,
          .invocation = &lock,
          .result = &result,
          .enactor_default = "You can't link to there."});
      return;
    }
    game_object_set_location(evaluation->world->database, exit, loc);
    notify_checked(evaluation, player, player, "Linked.", MSG_ME);
  }
}

void do_open(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *direction = invocation->first;
  int nlinks = invocation->vector_count;
  DbRef loc, destnum;
  char *dest;

  /*
   * Create the exit and link to the destination, if there is one
   */

  if (nlinks >= 1)
    dest = command_invocation_vector_at(invocation, 0);
  else
    dest = nullptr;

  if (key == OPEN_INVENTORY)
    loc = player;
  else
    loc = game_object_location(evaluation->world->database, player);

  open_exit(&(ExitCreationRequest){.evaluation = evaluation,
                                   .player = player,
                                   .location = loc,
                                   .direction = direction,
                                   .destination = dest});

  /*
   * Open the back link if we can
   */

  if (nlinks >= 2) {
    destnum = parse_linkable_room(evaluation, &invocation->context->match,
                                  player, dest);
    if (destnum != NOTHING) {
      open_exit(&(ExitCreationRequest){
          .evaluation = evaluation,
          .player = player,
          .location = destnum,
          .direction = command_invocation_vector_at(invocation, 1),
          .destination = tprintf("%ld", loc)});
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
      notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
      return;
    }
    if (!lock_test(evaluation, player, player, player, dest, LUA_LOCK_LINK,
                   LUA_LOCK_OPERATION_LINK, false, &lock, &result)) {
      notify_lock_failure(
          &(LockFailureNotification){.evaluation = evaluation,
                                     .invocation = &lock,
                                     .result = &result,
                                     .enactor_default = "Permission denied."});
      return;
    }
  }
  /*
   * Exit must be unlinked or controlled by you
   */

  if ((game_object_location(evaluation->world->database, exit) != NOTHING) &&
      !is_controls(evaluation->world->database, player, exit)) {
    notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    return;
  }
  /*
   * link has been validated and paid for, do it and tell the player
   */

  game_object_set_location(evaluation->world->database, exit, dest);
  notify_checked(evaluation, player, player, "Linked.", MSG_ME);
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
      notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
      break;
    }
    init_match(&invocation->context->match, player, where, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, MAT_NO_EXITS);
    room = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, room))
      break;
    if (!has_contents(evaluation->world->database, room)) {
      notify_checked(evaluation, player, player, "Can't link to an exit.",
                     MSG_ME);
      break;
    }
    if (!can_set_home(evaluation, player, thing, room)) {
      notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    } else if (!lock_test(evaluation, player, invocation->cause, player, room,
                          LUA_LOCK_LINK, LUA_LOCK_OPERATION_SET_HOME, false,
                          &lock, &result)) {
      notify_lock_failure(
          &(LockFailureNotification){.evaluation = evaluation,
                                     .invocation = &lock,
                                     .result = &result,
                                     .enactor_default = "Permission denied."});
    } else if (room == HOME) {
      notify_checked(evaluation, player, player, "Can't set home to home.",
                     MSG_ME);
    } else {
      game_object_set_link(evaluation->world->database, thing, room);
      notify_checked(evaluation, player, player, "Home set.", MSG_ME);
    }
    break;
  case OBJECT_TYPE_ROOM:

    /*
     * Set dropto
     */

    if (!is_controls(evaluation->world->database, player, thing)) {
      notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
      break;
    }
    room = parse_linkable_room(evaluation, &invocation->context->match, player,
                               where);
    if (!(is_good_obj(evaluation->world->database, room) || (room == HOME)))
      break;

    if ((room != HOME) && !is_room(evaluation->world->database, room)) {
      notify_checked(evaluation, player, player, "That is not a room!", MSG_ME);
    } else if ((room != HOME) &&
               !is_controls(evaluation->world->database, player, room)) {
      notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    } else if ((room != HOME) &&
               !lock_test(evaluation, player, invocation->cause, player, room,
                          LUA_LOCK_LINK, LUA_LOCK_OPERATION_LINK, false, &lock,
                          &result)) {
      notify_lock_failure(
          &(LockFailureNotification){.evaluation = evaluation,
                                     .invocation = &lock,
                                     .result = &result,
                                     .enactor_default = "Permission denied."});
    } else {
      game_object_set_location(evaluation->world->database, thing, room);
      notify_checked(evaluation, player, player, "Dropto set.", MSG_ME);
    }
    break;
  case OBJECT_TYPE_GARBAGE:
    notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    break;
  default:
    log_error((LogEntry){.log = evaluation->log,
                         .key = LOG_BUGS,
                         .primary = "BUG",
                         .secondary = "OTYPE"},
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
  int nargs = invocation->vector_count;
  DbRef room;
  char buff[SBUF_SIZE];
  char *compiled_name;

  /*
   * we don't need to know player's location!  hooray!
   */

  if (!name || !*name) {
    notify_checked(evaluation, player, player, "Dig what?", MSG_ME);
    return;
  }
  compiled_name = builder_compile_object_name(evaluation, player, name);
  if (!compiled_name)
    return;
  room = create_obj(evaluation, player, OBJECT_TYPE_ROOM, compiled_name);
  free_lbuf(compiled_name);
  if (room == NOTHING)
    return;

  notify_printf(evaluation, player, "%s created with room number %ld.",
                game_object_name(evaluation->world->database, room), room);

  char *forward_exit =
      nargs >= 1 ? command_invocation_vector_at(invocation, 0) : nullptr;
  char *back_exit =
      nargs >= 2 ? command_invocation_vector_at(invocation, 1) : nullptr;

  if (forward_exit != nullptr && *forward_exit) {
    (void)snprintf(buff, SBUF_SIZE, "%ld", room);
    open_exit(&(ExitCreationRequest){
        .evaluation = evaluation,
        .player = player,
        .location = game_object_location(evaluation->world->database, player),
        .direction = forward_exit,
        .destination = buff});
  }
  if (back_exit != nullptr && *back_exit) {
    (void)snprintf(buff, SBUF_SIZE, "%ld",
                   game_object_location(evaluation->world->database, player));
    open_exit(&(ExitCreationRequest){.evaluation = evaluation,
                                     .player = player,
                                     .location = room,
                                     .direction = back_exit,
                                     .destination = buff});
  }
  if (key == DIG_TELEPORT)
    (void)move_via_teleport(&(ObjectMovementRequest){.evaluation = evaluation,
                                                     .object = player,
                                                     .destination = room,
                                                     .cause = cause});
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
  compiled_name = builder_compile_object_name(evaluation, player, name);
  if (!compiled_name)
    return;
  styled_text_strip(evaluation->world->styled_text_palette, compiled_name,
                    clearbuffer, MBUF_SIZE);
  if (!name || !*name || (strlen(clearbuffer) == 0)) {
    notify_checked(evaluation, player, player, "Create what?", MSG_ME);
    free_lbuf(compiled_name);
    return;
  }
  thing = create_obj(evaluation, player, OBJECT_TYPE_THING, compiled_name);
  free_lbuf(compiled_name);
  if (thing == NOTHING)
    return;

  move_via_generic(&(ObjectMovementRequest){.evaluation = evaluation,
                                            .object = thing,
                                            .destination = player,
                                            .cause = NOTHING});
  game_object_set_link(evaluation->world->database, thing,
                       new_home(evaluation, player));
  notify_printf(evaluation, player, "%s created as object #%ld",
                game_object_name(invocation->context->world->database, thing),
                thing);
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
    notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    return;
  }
  if (is_player(evaluation->world->database, thing)) {
    notify_checked(evaluation, player, player, "You cannot clone players!",
                   MSG_ME);
    return;
  }
  if ((typeof_obj(evaluation->world->database, thing) == OBJECT_TYPE_EXIT) &&
      !is_controls(evaluation->world->database, player, loc)) {
    notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    return;
  }

  /*
   * Go make the clone object
   */

  if (arg2 && *arg2) {
    clone_name = builder_compile_object_name(evaluation, player, arg2);
    if (!clone_name)
      return;
    styled_text_strip(evaluation->world->styled_text_palette, clone_name,
                      pure_name, sizeof(pure_name));
    if (!ok_name(invocation->context->world->configuration, pure_name)) {
      notify_checked(evaluation, player, player,
                     "That is not a reasonable name.", MSG_ME);
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
  attribute_copy(&(AttributeCopyRequest){
      .evaluation = evaluation, .source = thing, .destination = clone});

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
  game_object_set_flag(
      &(ObjectFlagChangeRequest){.database = evaluation->world->database,
                                 .object = clone,
                                 .flag = OBJECT_FLAG_WIZARD});

  /*
   * Tell creator about it
   */

  if (arg2 && *arg2)
    notify_printf(evaluation, player,
                  "%s cloned as %s, new copy is object #%ld.",
                  game_object_name(invocation->context->world->database, thing),
                  arg2, clone);
  else
    notify_printf(evaluation, player, "%s cloned, new copy is object #%ld.",
                  game_object_name(invocation->context->world->database, thing),
                  clone);
  /*
   * Put the new thing in its new home.  Break any dropto or link, then
   * * * * * * * try to re-establish it.
   */

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_THING:
    game_object_set_link(
        evaluation->world->database, clone,
        clone_home(&(CloneHomeRequest){
            .evaluation = evaluation, .player = player, .source = thing}));
    move_via_generic(&(ObjectMovementRequest){.evaluation = evaluation,
                                              .object = clone,
                                              .destination = loc,
                                              .cause = player});
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

  newplayer = create_player(&(PlayerCreationRequest){
      .evaluation = evaluation, .name = name, .password = pass});
  if (newplayer == NOTHING) {
    notify_checked(evaluation, player, player,
                   tprintf("Failure creating '%s'", name), MSG_ME);
    return;
  }
  move_object(evaluation, newplayer,
              invocation->context->world->configuration->start_room);
  notify_checked(evaluation, player, player,
                 tprintf("New player '%s' (#%ld) created with password '%s'",
                         name, newplayer, pass),
                 MSG_ME);

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
