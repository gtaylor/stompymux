/*
 * look.c -- commands which look at things
 */

#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/world/access.h"
#include "mux/world/object_spatial.h"

#include "mux/commands/action_messages.h"
#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_runtime.h"
#include "mux/commands/look.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/walkdb.h"
#include "mux/world/world_context.h"

#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <math.h>

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

static void look_append_quoted_target(const char *target, char *buffer,
                                      char **cursor) {
  for (const char *scan = target; *scan; scan++) {
    if (*scan == '\\' || *scan == '"')
      safe_chr('\\', buffer, cursor);
    safe_chr(*scan, buffer, cursor);
  }
}

static void look_exit_parts(const StyledTextPalette *palette,
                            const char *stored_name, char *display,
                            char *command) {
  const char *primary_end = strchr(stored_name, ';');
  const char *command_start = stored_name;
  const char *command_end =
      primary_end ? primary_end : stored_name + strlen(stored_name);
  char raw_command[LBUF_SIZE];
  size_t display_size = (size_t)(command_end - stored_name);

  if (display_size >= LBUF_SIZE)
    display_size = LBUF_SIZE - 1;
  memcpy(display, stored_name, display_size);
  display[display_size] = '\0';

  if (primary_end && primary_end[1] != '\0' && primary_end[1] != ';') {
    command_start = primary_end + 1;
    command_end = strchr(command_start, ';');
    if (!command_end)
      command_end = command_start + strlen(command_start);
  }
  size_t command_size = (size_t)(command_end - command_start);
  if (command_size >= sizeof(raw_command))
    command_size = sizeof(raw_command) - 1;
  memcpy(raw_command, command_start, command_size);
  raw_command[command_size] = '\0';
  styled_text_strip(palette, raw_command, command, LBUF_SIZE);
}

static void look_append_exit_link(const char *display, const char *command,
                                  char *buffer, char **cursor) {
  safe_str("[send=\"", buffer, cursor);
  look_append_quoted_target(command, buffer, cursor);
  safe_str("\"]", buffer, cursor);
  safe_str(display, buffer, cursor);
  safe_str("[/]", buffer, cursor);
}

static void look_exits(EvaluationContext *evaluation, DbRef player, DbRef loc,
                       const char *exit_name) {
  WorldContext *world = evaluation->world;
  DbRef thing;
  char *buff, *e, *buff1, *command;
  int foundany, key;

  /*
   * make sure location has exits
   */

  if (!is_good_obj(evaluation->world->database, loc) ||
      !has_exits(evaluation->world->database, loc))
    return;

  /*
   * make sure there is at least one visible exit
   */

  foundany = 0;
  key = 0;
  if (is_dark(evaluation->world->database, loc))
    key |= VE_LOC_DARK;
  DOLIST(evaluation->world->database, thing,
         game_object_exits(evaluation->world->database, loc)) {
    if (exit_displayable(world->database, thing, player, key)) {
      foundany = 1;
      break;
    }
  }

  if (!foundany)
    return;
  /*
   * Display the list of exit names
   */

  notify_checked(evaluation, player, player, exit_name,
                 MSG_ME_ALL | MSG_F_DOWN);
  e = buff = alloc_lbuf("look_exits");
  buff1 = alloc_lbuf("look_exits2");
  command = alloc_lbuf("look_exits.command");
  if (is_transparent(evaluation->world->database, loc)) {
    DOLIST(evaluation->world->database, thing,
           game_object_exits(evaluation->world->database, loc)) {
      if (exit_displayable(world->database, thing, player, key)) {
        e = buff;
        look_exit_parts(evaluation->world->styled_text_palette,
                        game_object_name(evaluation->world->database, thing),
                        buff1, command);
        look_append_exit_link(buff1, command, buff, &e);
        *e = '\0';
        notify_printf(
            evaluation, player, "%s leads to %s.", buff,
            game_object_name(
                evaluation->world->database,
                game_object_location(evaluation->world->database, thing)));
      }
    }
  } else {
    DOLIST(evaluation->world->database, thing,
           game_object_exits(evaluation->world->database, loc)) {
      if (exit_displayable(world->database, thing, player, key)) {
        if (buff != e)
          safe_str("  ", buff, &e);
        look_exit_parts(evaluation->world->styled_text_palette,
                        game_object_name(evaluation->world->database, thing),
                        buff1, command);
        look_append_exit_link(buff1, command, buff, &e);
      }
    }
  }

  if (!(is_transparent(evaluation->world->database, loc))) {
    safe_str("\r\n", buff, &e);
    *e = 0;
    notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
  }
  free_lbuf(buff);
  free_lbuf(buff1);
  free_lbuf(command);
}

#define CONTENTS_LOCAL 0
#define CONTENTS_NESTED 1

static void look_contents(EvaluationContext *evaluation, DbRef player,
                          DbRef loc, const char *contents_name, int style) {
  DbRef thing;
  int can_see_loc;
  char *buff;

  /*
   * check to see if he can see the location
   */

  can_see_loc = !is_dark(evaluation->world->database, loc);

  /*
   * check to see if there is anything there
   */

  DOLIST(evaluation->world->database, thing,
         game_object_contents(evaluation->world->database, loc)) {
    if (can_see(evaluation, player, thing, can_see_loc)) {

      /*
       * something exists!  show him everything
       */

      notify_checked(evaluation, player, player, contents_name,
                     MSG_ME_ALL | MSG_F_DOWN);
      DOLIST(evaluation->world->database, thing,
             game_object_contents(evaluation->world->database, loc)) {
        if (can_see(evaluation, player, thing, can_see_loc)) {
          buff = unparse_object(evaluation->world->database, evaluation, player,
                                thing);
          notify_checked(evaluation, player, player, buff,
                         MSG_ME_ALL | MSG_F_DOWN);
          free_lbuf(buff);
        }
      }
      break; /*
              * we're done
              */
    }
  }
}

static bool look_custom_appearance(EvaluationContext *evaluation, DbRef player,
                                   DbRef thing) {
  LuaAppearanceResult result;
  const LuaAppearanceType type =
      is_room(evaluation->world->database, thing) ||
              game_object_location(evaluation->world->database, player) == thing
          ? LUA_APPEARANCE_INTERNAL
          : LUA_APPEARANCE_EXTERNAL;

  lua_appearance_evaluate(
      evaluation->runtime->lua_owner->runtime,
      &(LuaAppearanceInvocation){
          .type = type,
          .descriptor =
              evaluation->command ? evaluation->command->descriptor : nullptr,
          .object = thing,
          .enactor = player,
          .cause = player,
      },
      &result);
  if (!result.defined)
    return false;
  if (*result.rendered)
    notify_checked(evaluation, player, player, result.rendered,
                   MSG_ME_ALL | MSG_F_DOWN);
  notify_event(evaluation,
               evaluation->command ? evaluation->command->descriptor : nullptr,
               player, player, thing, LUA_EVENT_DESCRIBE, nullptr, 0);
  return true;
}

static bool look_simple(EvaluationContext *evaluation, DbRef player,
                        DbRef thing) {
  int pattr;
  char *buff;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(evaluation, player))
    return false;

  if (look_custom_appearance(evaluation, player, thing))
    return true;

  /*
   * Get the name and db-number if we can examine it.
   */

  if (is_examinable(evaluation->world->database, player, thing)) {
    buff =
        unparse_object(evaluation->world->database, evaluation, player, thing);
    notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
    free_lbuf(buff);
  }
  pattr = A_DESC;
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_DESCRIBE,
                                .operation = LUA_MESSAGE_OPERATION_DESCRIBE,
                                .object = thing,
                                .enactor = player,
                                .cause = player,
                                .source = NOTHING,
                                .destination = NOTHING},
                    .content_attribute = pattr,
                    .enactor_default = "You see nothing special.",
                    .event = LUA_EVENT_DESCRIBE});
  return false;
}

static void show_a_desc(EvaluationContext *evaluation, DbRef player,
                        DbRef loc) {
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_DESCRIBE,
                                .operation = LUA_MESSAGE_OPERATION_DESCRIBE,
                                .object = loc,
                                .enactor = player,
                                .cause = player,
                                .source = NOTHING,
                                .destination = NOTHING},
                    .content_attribute = A_DESC,
                    .event = LUA_EVENT_DESCRIBE});
}

static void show_desc(EvaluationContext *evaluation, DbRef player, DbRef loc,
                      int use_idesc) {
  char *got;
  long aflags;

  if ((typeof_obj(evaluation->world->database, loc) != OBJECT_TYPE_ROOM) &&
      use_idesc) {
    if (*(got = attribute_get(evaluation->world->database, loc, A_IDESC,
                              &aflags)))
      notify_action(
          evaluation,
          &(ActionMessageInvocation){
              .message = {.type = LUA_MESSAGE_DESCRIBE,
                          .operation = LUA_MESSAGE_OPERATION_INSIDE_DESCRIBE,
                          .object = loc,
                          .enactor = player,
                          .cause = player,
                          .source = NOTHING,
                          .destination = NOTHING},
              .content_attribute = A_IDESC,
              .event = LUA_EVENT_DESCRIBE});
    else
      show_a_desc(evaluation, player, loc);
    free_lbuf(got);
  } else {
    show_a_desc(evaluation, player, loc);
  }
}

void look_in(EvaluationContext *evaluation, DbRef player, DbRef loc, int key) {
  char *buff;
  bool custom;
  LuaLockInvocation lock;
  LuaLockResult result;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(evaluation, player))
    return;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  custom = look_custom_appearance(evaluation, player, loc);
  if (!custom) {
    buff = unparse_object(evaluation->world->database, evaluation, player, loc);
    notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
    free_lbuf(buff);

    show_desc(evaluation, player, loc,
              loc == game_object_location(evaluation->world->database, player));
  }

  /*
   * tell him the appropriate messages if he has the key
   */

  if (typeof_obj(evaluation->world->database, loc) == OBJECT_TYPE_ROOM) {
    if (lock_test(evaluation, player, player, player, loc, LUA_LOCK_DEFAULT,
                  LUA_LOCK_OPERATION_LOOK, false, &lock, &result))
      notify_action(evaluation,
                    &(ActionMessageInvocation){
                        .message = {.type = LUA_MESSAGE_SUCCESS,
                                    .operation = LUA_MESSAGE_OPERATION_LOOK,
                                    .object = loc,
                                    .enactor = player,
                                    .cause = player,
                                    .source = NOTHING,
                                    .destination = NOTHING},
                        .event = LUA_EVENT_SUCCESS});
    else
      notify_lock_failure(evaluation, &lock, &result, nullptr, nullptr,
                          LUA_EVENT_FAIL);
  }
  if (custom)
    return;
  /*
   * tell him the attributes, contents and exits
   */

  look_contents(evaluation, player, loc, "Contents:", CONTENTS_LOCAL);
  if (key & LK_SHOWEXIT)
    look_exits(evaluation, player, loc, "Obvious exits:");
}

void do_look(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *name = invocation->first;
  DbRef thing, loc;
  int look_key;

  look_key = LK_SHOWATTR | LK_SHOWEXIT;

  loc = game_object_location(evaluation->world->database, player);
  if (!name || !*name) {
    thing = loc;
    if (is_good_obj(evaluation->world->database, thing)) {
      if (key & LOOK_OUTSIDE) {
        if (typeof_obj(evaluation->world->database, thing) ==
            OBJECT_TYPE_ROOM) {
          notify_checked(evaluation, player, player, "You can't look outside.",
                         MSG_ME);
          return;
        }
        thing = game_object_location(evaluation->world->database, thing);
      }
      look_in(evaluation, player, thing, look_key);
    }
    return;
  }
  /*
   * Look for the target locally
   */

  thing = (key & LOOK_OUTSIDE) ? loc : player;
  init_match(&invocation->context->match, thing, name, OBJECT_TYPE_NOTYPE);
  match_exit(&invocation->context->match);
  match_neighbor(&invocation->context->match);
  match_possession(&invocation->context->match);
  match_here(&invocation->context->match);
  match_me(&invocation->context->match);
  thing = match_result(&invocation->context->match);

  /*
   * Not found locally, check possessive
   */

  if (!is_good_obj(evaluation->world->database, thing)) {
    thing = match_status(evaluation, player,
                         match_possessed(&invocation->context->match, player,
                                         ((key & LOOK_OUTSIDE) ? loc : player),
                                         (char *)name, thing));
  }
  /*
   * If we found something, go handle it
   */

  if (is_good_obj(evaluation->world->database, thing)) {
    switch (typeof_obj(evaluation->world->database, thing)) {
    case OBJECT_TYPE_ROOM:
      look_in(evaluation, player, thing, look_key);
      break;
    case OBJECT_TYPE_THING:
    case OBJECT_TYPE_PLAYER:
      if (!look_simple(evaluation, player, thing)) {
        look_contents(evaluation, player, thing, "Carrying:", CONTENTS_NESTED);
      }
      break;
    case OBJECT_TYPE_EXIT:
      if (!look_simple(evaluation, player, thing) &&
          is_transparent(evaluation->world->database, thing) &&
          (game_object_location(evaluation->world->database, thing) !=
           NOTHING)) {
        look_key &= ~LK_SHOWATTR;
        look_in(evaluation, player,
                game_object_location(evaluation->world->database, thing),
                look_key);
      }
      break;
    default:
      (void)look_simple(evaluation, player, thing);
    }
  }
}
