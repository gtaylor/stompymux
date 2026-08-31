/*
 * look.c -- commands which look at things
 */

#include <string.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/look.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h" // IWYU pragma: keep
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/owned_text.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/object_spatial.h"

static void look_append_quoted_target(const char *target, char *buffer,
                                      char **cursor) {
  const size_t LENGTH = strlen(target);

  for (size_t index = 0; index < LENGTH; index++) {
    const char CHARACTER = *(const char *)checked_storage_at_const(
        target, LENGTH + 1, sizeof(char), index);

    if (CHARACTER == '\\' || CHARACTER == '"')
      safe_chr('\\', buffer, cursor);
    safe_chr(CHARACTER, buffer, cursor);
  }
}

typedef struct LookExitPartsRequest {
  const StyledTextPalette *palette;
  const char *stored_name;
  char *display;
  char *command;
} LookExitPartsRequest;

static void look_exit_parts(const LookExitPartsRequest *request) {
  const StyledTextPalette *palette = request->palette;
  const char *stored_name = request->stored_name;
  char *display = request->display;
  char *command = request->command;
  const size_t STORED_LENGTH = strlen(stored_name);
  size_t primary_end = 0;
  size_t command_start = 0;
  size_t command_end;
  char raw_command[LBUF_SIZE];

  while (primary_end < STORED_LENGTH &&
         *(const char *)checked_storage_at_const(
             stored_name, STORED_LENGTH + 1, sizeof(char), primary_end) != ';')
    primary_end++;
  command_end = primary_end;
  size_t display_size = command_end;

  if (display_size >= LBUF_SIZE)
    display_size = LBUF_SIZE - 1;
  memcpy(display, stored_name, display_size);
  *(char *)checked_storage_at(display, LBUF_SIZE, sizeof(char), display_size) =
      '\0';

  if (primary_end < STORED_LENGTH && primary_end + 1 < STORED_LENGTH &&
      *(const char *)checked_storage_at_const(stored_name, STORED_LENGTH + 1,
                                              sizeof(char),
                                              primary_end + 1) != ';') {
    command_start = primary_end + 1;
    command_end = command_start;
    while (command_end < STORED_LENGTH &&
           *(const char *)checked_storage_at_const(
               stored_name, STORED_LENGTH + 1, sizeof(char), command_end) !=
               ';')
      command_end++;
  }
  size_t command_size = command_end - command_start;
  if (command_size >= sizeof(raw_command))
    command_size = sizeof(raw_command) - 1;
  memcpy(raw_command, checked_string_suffix(stored_name, command_start),
         command_size);
  *(char *)checked_storage_at(raw_command, sizeof(raw_command), sizeof(char),
                              command_size) = '\0';
  styled_text_strip(palette, raw_command, command, LBUF_SIZE);
}

typedef struct LookExitLinkRequest {
  const char *display;
  const char *command;
  char *buffer;
  char **cursor;
} LookExitLinkRequest;

static void look_append_exit_link(const LookExitLinkRequest *request) {
  safe_str("[send=\"", request->buffer, request->cursor);
  look_append_quoted_target(request->command, request->buffer, request->cursor);
  safe_str("\"]", request->buffer, request->cursor);
  safe_str(request->display, request->buffer, request->cursor);
  safe_str("[/]", request->buffer, request->cursor);
}

typedef struct LookContext {
  EvaluationContext *evaluation;
  DbRef viewer;
  DbRef location;
} LookContext;

static void look_exits(const LookContext *look, const char *exit_name) {
  EvaluationContext *evaluation = look->evaluation;
  DbRef player = look->viewer;
  DbRef loc = look->location;
  WorldContext *world = evaluation->world;
  DbRef thing;
  char *buff;
  char *e;
  char *buff1;
  char *command;
  int foundany;
  int key;

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
    if (exit_displayable(&(ExitVisibilityRequest){.database = world->database,
                                                  .exit = thing,
                                                  .viewer = player,
                                                  .options = key})) {
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
      if (exit_displayable(&(ExitVisibilityRequest){.database = world->database,
                                                    .exit = thing,
                                                    .viewer = player,
                                                    .options = key})) {
        e = buff;
        look_exit_parts(&(LookExitPartsRequest){
            .palette = evaluation->world->styled_text_palette,
            .stored_name = game_object_name(evaluation->world->database, thing),
            .display = buff1,
            .command = command});
        look_append_exit_link(&(LookExitLinkRequest){.display = buff1,
                                                     .command = command,
                                                     .buffer = buff,
                                                     .cursor = &e});
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
      if (exit_displayable(&(ExitVisibilityRequest){.database = world->database,
                                                    .exit = thing,
                                                    .viewer = player,
                                                    .options = key})) {
        if (buff != e)
          safe_str("  ", buff, &e);
        look_exit_parts(&(LookExitPartsRequest){
            .palette = evaluation->world->styled_text_palette,
            .stored_name = game_object_name(evaluation->world->database, thing),
            .display = buff1,
            .command = command});
        look_append_exit_link(&(LookExitLinkRequest){.display = buff1,
                                                     .command = command,
                                                     .buffer = buff,
                                                     .cursor = &e});
      }
    }
  }

  if (!(is_transparent(evaluation->world->database, loc))) {
    safe_str("\r\n", buff, &e);
    *e = 0;
    notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
  }
  free_buf(buff);
  free_buf(buff1);
  free_buf(command);
}

enum ContentsStyle : int {
  CONTENTS_LOCAL = 0,
  CONTENTS_NESTED = 1,
};

static void look_contents(const LookContext *look, const char *contents_name,
                          int style [[maybe_unused]]) {
  EvaluationContext *evaluation = look->evaluation;
  DbRef player = look->viewer;
  DbRef loc = look->location;
  DbRef thing;
  int can_see_loc;
  OwnedText buff;

  /*
   * check to see if he can see the location
   */

  can_see_loc = !is_dark(evaluation->world->database, loc);

  /*
   * check to see if there is anything there
   */

  DOLIST(evaluation->world->database, thing,
         game_object_contents(evaluation->world->database, loc)) {
    if (can_see(
            &(ObjectVisibilityRequest){.evaluation = evaluation,
                                       .viewer = player,
                                       .object = thing,
                                       .location_visible = can_see_loc != 0})) {

      /*
       * something exists!  show him everything
       */

      notify_checked(evaluation, player, player, contents_name,
                     MSG_ME_ALL | MSG_F_DOWN);
      DOLIST(evaluation->world->database, thing,
             game_object_contents(evaluation->world->database, loc)) {
        if (can_see(&(ObjectVisibilityRequest){.evaluation = evaluation,
                                               .viewer = player,
                                               .object = thing,
                                               .location_visible =
                                                   can_see_loc != 0})) {
          buff = unparse_object(evaluation->world->database, evaluation, player,
                                thing);
          notify_checked(evaluation, player, player, buff.text,
                         MSG_ME_ALL | MSG_F_DOWN);
          owned_text_release(&buff);
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
  const LuaAppearanceType TYPE =
      is_room(evaluation->world->database, thing) ||
              game_object_location(evaluation->world->database, player) == thing
          ? LUA_APPEARANCE_INTERNAL
          : LUA_APPEARANCE_EXTERNAL;

  lua_appearance_evaluate(
      evaluation->runtime->lua_owner->runtime,
      &(LuaAppearanceInvocation){
          .type = TYPE,
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
  OwnedText buff;

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
    notify_checked(evaluation, player, player, buff.text,
                   MSG_ME_ALL | MSG_F_DOWN);
    owned_text_release(&buff);
  }
  pattr = A_DESCRIPTION;
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
                    .content_attribute = A_DESCRIPTION,
                    .event = LUA_EVENT_DESCRIBE});
}

static void show_desc(EvaluationContext *evaluation, DbRef player, DbRef loc,
                      int use_idesc) {
  OwnedText got;
  long aflags;

  if ((typeof_obj(evaluation->world->database, loc) != OBJECT_TYPE_ROOM) &&
      use_idesc) {
    got = attribute_get(evaluation->world->database, loc,
                        A_INTERNAL_DESCRIPTION, &aflags);
    if (*got.text) {
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
              .content_attribute = A_INTERNAL_DESCRIPTION,
              .event = LUA_EVENT_DESCRIBE});
    } else {
      show_a_desc(evaluation, player, loc);
    }
    owned_text_release(&got);
  } else {
    show_a_desc(evaluation, player, loc);
  }
}

void look_in(const LookRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->viewer;
  DbRef loc = request->location;
  int key = request->key;
  OwnedText buff;
  bool custom;

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
    notify_checked(evaluation, player, player, buff.text,
                   MSG_ME_ALL | MSG_F_DOWN);
    owned_text_release(&buff);

    show_desc(evaluation, player, loc,
              loc == game_object_location(evaluation->world->database, player));
  }

  if (custom)
    return;
  /*
   * tell him the attributes, contents and exits
   */

  LookContext look = {
      .evaluation = evaluation, .viewer = player, .location = loc};
  look_contents(&look, "Contents:", CONTENTS_LOCAL);
  if (key & LK_SHOWEXIT)
    look_exits(&look, "Obvious exits:");
}

void do_look(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef PLAYER = invocation->player;
  const int KEY = invocation->key;
  char *name = invocation->first;
  DbRef thing;
  DbRef loc;
  int look_key;

  look_key = LK_SHOWATTR | LK_SHOWEXIT;

  loc = game_object_location(evaluation->world->database, PLAYER);
  if (!name || !*name) {
    thing = loc;
    if (is_good_obj(evaluation->world->database, thing)) {
      if (KEY & LOOK_OUTSIDE) {
        if (typeof_obj(evaluation->world->database, thing) ==
            OBJECT_TYPE_ROOM) {
          notify_checked(evaluation, PLAYER, PLAYER, "You can't look outside.",
                         MSG_ME);
          return;
        }
        thing = game_object_location(evaluation->world->database, thing);
      }
      look_in(&(LookRequest){.evaluation = evaluation,
                             .viewer = PLAYER,
                             .location = thing,
                             .key = look_key});
    }
    return;
  }
  /*
   * Look for the target locally
   */

  thing = (KEY & LOOK_OUTSIDE) ? loc : PLAYER;
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
    thing = match_status(evaluation, PLAYER,
                         match_possessed(&invocation->context->match, PLAYER,
                                         ((KEY & LOOK_OUTSIDE) ? loc : PLAYER),
                                         name, thing));
  }
  /*
   * If we found something, go handle it
   */

  if (is_good_obj(evaluation->world->database, thing)) {
    switch (typeof_obj(evaluation->world->database, thing)) {
    case OBJECT_TYPE_ROOM:
      look_in(&(LookRequest){.evaluation = evaluation,
                             .viewer = PLAYER,
                             .location = thing,
                             .key = look_key});
      break;
    case OBJECT_TYPE_THING:
    case OBJECT_TYPE_PLAYER:
      if (!look_simple(evaluation, PLAYER, thing)) {
        LookContext look = {
            .evaluation = evaluation, .viewer = PLAYER, .location = thing};
        look_contents(&look, "Carrying:", CONTENTS_NESTED);
      }
      break;
    case OBJECT_TYPE_EXIT:
      if (!look_simple(evaluation, PLAYER, thing) &&
          is_transparent(evaluation->world->database, thing) &&
          (game_object_location(evaluation->world->database, thing) !=
           NOTHING)) {
        look_key &= ~LK_SHOWATTR;
        look_in(&(LookRequest){.evaluation = evaluation,
                               .viewer = PLAYER,
                               .location = game_object_location(
                                   evaluation->world->database, thing),
                               .key = look_key});
      }
      break;
    default:
      (void)look_simple(evaluation, PLAYER, thing);
    }
  }
}
