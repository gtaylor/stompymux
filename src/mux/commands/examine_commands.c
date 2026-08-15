/*
 * look.c -- commands which look at things
 */

#include "mux/commands/examine_commands.h"

#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_keys.h"
#include "mux/commands/state_commands.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/powers.h"
#include "mux/persistence/gamedb.h" // IWYU pragma: keep
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include <stddef.h>
#include <stdio.h>
#include <string.h>

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

typedef struct ExamineMarkupRequest {
  EvaluationContext *evaluation;
  DbRef viewer;
  const char *label;
  const char *styled;
} ExamineMarkupRequest;

static void examine_notify_markup(const ExamineMarkupRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->viewer;
  char *markup = alloc_lbuf("examine_notify_markup");

  if (!styled_text_escape(request->styled, markup, LBUF_SIZE))
    styled_text_strip(evaluation->world->styled_text_palette, request->styled,
                      markup, LBUF_SIZE);
  if (request->label)
    notify_printf(evaluation, player, "%s: %s", request->label, markup);
  else
    notify_checked(evaluation, player, player, markup, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(markup);
}

static void examine_notify_indented(EvaluationContext *evaluation, DbRef player,
                                    const char *text) {
  char *buffer = alloc_lbuf("examine_notify_indented");

  (void)snprintf(buffer, LBUF_SIZE, "  %s", text);
  notify_checked(evaluation, player, player, buffer, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(buffer);
}

static void debug_examine(EvaluationContext *evaluation, DbRef player,
                          DbRef thing) {
  char *buf;

  notify_printf(evaluation, player, "Number  = %ld", thing);
  if (!is_good_obj(evaluation->world->database, thing))
    return;

  notify_printf(evaluation, player, "Name    = %s",
                game_object_name(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Location= %ld",
                game_object_location(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Contents= %ld",
                game_object_contents(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Exits   = %ld",
                game_object_exits(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Link    = %ld",
                game_object_link(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Next    = %ld",
                game_object_next(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Zone    = %ld",
                game_object_zone(evaluation->world->database, thing));
  buf = flag_description(evaluation->world->database, thing);
  notify_printf(evaluation, player, "Flags   = %s", buf);
  free_mbuf(buf);
  buf = power_description(
      &(PowerDescriptionRequest){.database = evaluation->world->database,
                                 .viewer = player,
                                 .target = thing});
  notify_printf(evaluation, player, "Powers  = %s", buf);
  free_mbuf(buf);
  notify_printf(evaluation, player, "Lua state entries: %zu",
                object_state_count(evaluation->world->database, thing));
}

typedef struct ExamineObjectRequest {
  EvaluationContext *evaluation;
  DbRef viewer;
  DbRef object;
} ExamineObjectRequest;

static void examine_native_attributes(const ExamineObjectRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->viewer;
  DbRef thing = request->object;
  GameDatabase *database = evaluation->world->database;
  bool has_attributes = false;

  for (size_t index = 0; index < native_attribute_count(); index++) {
    const Attribute *entry = native_attribute_at(index);

    const char *value;

    if (!object_attribute_is_administrable(entry->number))
      continue;
    value = attribute_get_raw(database, thing, entry->number);
    if (!value || !*value)
      continue;
    if (!has_attributes) {
      notify_checked(evaluation, player, player,
                     "Attributes:", MSG_ME_ALL | MSG_F_DOWN);
      has_attributes = true;
    }
    char label[MBUF_SIZE];

    (void)snprintf(label, sizeof label, "  %s", entry->name);
    examine_notify_markup(&(ExamineMarkupRequest){.evaluation = evaluation,
                                                  .viewer = player,
                                                  .label = label,
                                                  .styled = value});
  }
}

void do_examine(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef PLAYER = invocation->player;
  const int KEY = invocation->key;
  char *name = invocation->first;
  DbRef thing;
  DbRef content;
  DbRef exit;
  DbRef loc;
  char *buf2;

  /*
   * This command is pointless if the player can't hear.
   */

  if (!is_hearer(evaluation, PLAYER))
    return;

  if (!name || !*name) {
    thing = game_object_location(evaluation->world->database, PLAYER);
    if (thing == NOTHING)
      return;
  } else {
    /* Look it up */

    init_match(&invocation->context->match, PLAYER, name, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  /*
   * Check for the /debug switch
   */

  if (KEY == EXAM_DEBUG) {
    debug_examine(evaluation, PLAYER, thing);
    return;
  }

  buf2 = unparse_object(evaluation->world->database, evaluation, PLAYER, thing);
  examine_notify_markup(&(ExamineMarkupRequest){
      .evaluation = evaluation, .viewer = PLAYER, .styled = buf2});
  free_lbuf(buf2);
  notify_printf(
      evaluation, PLAYER, "Type: %s",
      object_type_entry(typeof_obj(evaluation->world->database, thing))->name);
  buf2 = flags_description(evaluation->world->database, thing);
  notify_checked(evaluation, PLAYER, PLAYER, buf2, MSG_ME_ALL | MSG_F_DOWN);
  free_mbuf(buf2);

  buf2 = power_description(
      &(PowerDescriptionRequest){.database = evaluation->world->database,
                                 .viewer = PLAYER,
                                 .target = thing});
  notify_checked(evaluation, PLAYER, PLAYER, buf2, MSG_ME_ALL | MSG_F_DOWN);
  free_mbuf(buf2);
  examine_native_attributes(&(ExamineObjectRequest){
      .evaluation = evaluation, .viewer = PLAYER, .object = thing});
  buf2 = unparse_object(evaluation->world->database, evaluation, PLAYER,
                        game_object_zone(evaluation->world->database, thing));
  notify_printf(evaluation, PLAYER, "Zone: %s", buf2);
  free_lbuf(buf2);
  lua_examine_object(&(LuaExamineObjectRequest){
      .runtime = invocation->context->runtime->lua_owner->runtime,
      .evaluation = evaluation,
      .viewer = PLAYER,
      .object = thing});
  if (!(KEY & EXAM_BRIEF))
    state_examine_namespaces(&(ObjectStateExamineRequest){
        .evaluation = evaluation, .viewer = PLAYER, .object = thing});
  /*
   * show him interesting stuff
   */

  /*
   * Contents
   */

  if (game_object_contents(evaluation->world->database, thing) != NOTHING) {
    notify_checked(evaluation, PLAYER, PLAYER,
                   "Contents:", MSG_ME_ALL | MSG_F_DOWN);
    DOLIST(evaluation->world->database, content,
           game_object_contents(evaluation->world->database, thing)) {
      buf2 = unparse_object(evaluation->world->database, evaluation, PLAYER,
                            content);
      examine_notify_indented(evaluation, PLAYER, buf2);
      free_lbuf(buf2);
    }
  }
  /*
   * Show stuff that depends on the object type
   */

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_ROOM:

    /*
     * tell him about exits
     */

    if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
      notify_checked(evaluation, PLAYER, PLAYER,
                     "Exits:", MSG_ME_ALL | MSG_F_DOWN);
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, PLAYER,
                              exit);
        examine_notify_indented(evaluation, PLAYER, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify_checked(evaluation, PLAYER, PLAYER, "No exits.",
                     MSG_ME_ALL | MSG_F_DOWN);
    }

    /*
     * print dropto if present
     */

    if (game_object_location(evaluation->world->database, thing) != NOTHING) {
      buf2 = unparse_object(
          evaluation->world->database, evaluation, PLAYER,
          game_object_location(evaluation->world->database, thing));
      notify_printf(evaluation, PLAYER, "Dropped objects go to: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case OBJECT_TYPE_THING:
  case OBJECT_TYPE_PLAYER:

    /*
     * tell him about exits
     */

    if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
      notify_checked(evaluation, PLAYER, PLAYER,
                     "Exits:", MSG_ME_ALL | MSG_F_DOWN);
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, PLAYER,
                              exit);
        examine_notify_indented(evaluation, PLAYER, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify_checked(evaluation, PLAYER, PLAYER, "No exits.",
                     MSG_ME_ALL | MSG_F_DOWN);
    }

    /*
     * print home
     */

    loc = game_object_link(evaluation->world->database, thing);
    buf2 = unparse_object(evaluation->world->database, evaluation, PLAYER, loc);
    notify_printf(evaluation, PLAYER, "Home: %s", buf2);
    free_lbuf(buf2);

    /*
     * print location if player can link to it
     */

    loc = game_object_location(evaluation->world->database, thing);
    if (loc != NOTHING) {
      buf2 =
          unparse_object(evaluation->world->database, evaluation, PLAYER, loc);
      notify_printf(evaluation, PLAYER, "Location: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case OBJECT_TYPE_EXIT:
    buf2 =
        unparse_object(evaluation->world->database, evaluation, PLAYER,
                       game_object_exits(evaluation->world->database, thing));
    notify_printf(evaluation, PLAYER, "Source: %s", buf2);
    free_lbuf(buf2);

    /*
     * print destination
     */

    switch (game_object_location(evaluation->world->database, thing)) {
    case NOTHING:
      break;
    case HOME:
      notify_checked(evaluation, PLAYER, PLAYER, "Destination: *HOME*",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    default:
      buf2 = unparse_object(
          evaluation->world->database, evaluation, PLAYER,
          game_object_location(evaluation->world->database, thing));
      notify_printf(evaluation, PLAYER, "Destination: %s", buf2);
      free_lbuf(buf2);
      break;
    }
    break;
  default:
    break;
  }
}

void do_inventory(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef PLAYER = invocation->player;
  DbRef thing;
  char *buff;
  const char *s;
  char *e;

  thing = game_object_contents(evaluation->world->database, PLAYER);
  if (thing == NOTHING) {
    notify_checked(evaluation, PLAYER, PLAYER, "You aren't carrying anything.",
                   MSG_ME_ALL | MSG_F_DOWN);
  } else {
    notify_checked(evaluation, PLAYER, PLAYER,
                   "You are carrying:", MSG_ME_ALL | MSG_F_DOWN);
    DOLIST(evaluation->world->database, thing, thing) {
      buff = unparse_object(evaluation->world->database, evaluation, PLAYER,
                            thing);
      notify_checked(evaluation, PLAYER, PLAYER, buff, MSG_ME_ALL | MSG_F_DOWN);
      free_lbuf(buff);
    }
  }

  thing = game_object_exits(evaluation->world->database, PLAYER);
  if (thing != NOTHING) {
    notify_checked(evaluation, PLAYER, PLAYER,
                   "Exits:", MSG_ME_ALL | MSG_F_DOWN);
    e = buff = alloc_lbuf("look_exits");
    DOLIST(evaluation->world->database, thing, thing) {
      /*
       * chop off first exit alias to display
       */
      s = game_object_name(evaluation->world->database, thing);
      const size_t NAME_LENGTH = strlen(s);

      for (size_t index = 0; index < NAME_LENGTH; index++) {
        const char CHARACTER = *(const char *)checked_storage_at_const(
            s, NAME_LENGTH + 1, sizeof(char), index);

        if (CHARACTER == ';')
          break;
        safe_chr(CHARACTER, buff, &e);
      }
      safe_str("  ", buff, &e);
    }
    *e = 0;
    notify_checked(evaluation, PLAYER, PLAYER, buff, MSG_ME_ALL | MSG_F_DOWN);
    free_lbuf(buff);
  }
}

void do_entrances(CommandInvocation *invocation) {
  WorldContext *world = invocation->context->world;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef PLAYER = invocation->player;
  char *name = invocation->first;
  DbRef thing;
  DbRef i;
  char *exit;
  char *message;
  int control_thing;
  int count;
  long low_bound;
  long high_bound;

  parse_range(world->database, world->configuration, &name, &low_bound,
              &high_bound);
  if (!name || !*name) {
    if (has_location(evaluation->world->database, PLAYER))
      thing = game_object_location(evaluation->world->database, PLAYER);
    else
      thing = PLAYER;
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  } else {
    init_match(&invocation->context->match, PLAYER, name, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  message = alloc_lbuf("do_entrances");
  control_thing = is_examinable(evaluation->world->database, PLAYER, thing);
  count = 0;
  for (i = low_bound; i <= high_bound; i++) {
    if (control_thing ||
        is_examinable(evaluation->world->database, PLAYER, i)) {
      switch (typeof_obj(evaluation->world->database, i)) {
      case OBJECT_TYPE_EXIT:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit =
              unparse_object(evaluation->world->database, evaluation, PLAYER,
                             game_object_exits(evaluation->world->database, i));
          notify_printf(evaluation, PLAYER, "%s (%s)", exit,
                        game_object_name(evaluation->world->database, i));
          free_lbuf(exit);
          count++;
        }
        break;
      case OBJECT_TYPE_ROOM:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, PLAYER,
                                i);
          notify_printf(evaluation, PLAYER, "%s [dropto]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      case OBJECT_TYPE_THING:
      case OBJECT_TYPE_PLAYER:
        if (game_object_link(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, PLAYER,
                                i);
          notify_printf(evaluation, PLAYER, "%s [home]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      default:
        break;
      }
    }
  }
  free_lbuf(message);
  notify_printf(evaluation, PLAYER, "%d entrance%s found.", count,
                (count == 1) ? "" : "s");
}

/*
 * check the current location for bugs
 */
