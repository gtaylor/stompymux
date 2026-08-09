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

static void examine_notify_markup(EvaluationContext *evaluation, DbRef player,
                                  const char *label, const char *styled) {
  char *markup = alloc_lbuf("examine_notify_markup");

  if (!styled_text_escape(styled, markup, LBUF_SIZE))
    styled_text_strip(evaluation->world->styled_text_palette, styled, markup,
                      LBUF_SIZE);
  if (label)
    notify_printf(evaluation, player, "%s: %s", label, markup);
  else
    notify_checked(evaluation, player, player, markup, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(markup);
}

static void examine_notify_indented(EvaluationContext *evaluation, DbRef player,
                                    const char *text) {
  char *buffer = alloc_lbuf("examine_notify_indented");

  snprintf(buffer, LBUF_SIZE, "  %s", text);
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
  buf = flag_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Flags   = %s", buf);
  free_mbuf(buf);
  buf = power_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Powers  = %s", buf);
  free_mbuf(buf);
  notify_printf(evaluation, player, "Lua state entries: %zu",
                object_state_count(evaluation->world->database, thing));
}

static void examine_native_attributes(EvaluationContext *evaluation,
                                      DbRef player, DbRef thing) {
  GameDatabase *database = evaluation->world->database;
  bool has_attributes = false;

  for (size_t index = 0; index < native_attribute_count(); index++) {
    Attribute *entry = native_attribute_at(index);

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

    snprintf(label, sizeof label, "  %s", entry->name);
    examine_notify_markup(evaluation, player, label, value);
  }
}

void do_examine(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *name = invocation->first;
  DbRef thing, content, exit, loc;
  char *buf2;

  /*
   * This command is pointless if the player can't hear.
   */

  if (!is_hearer(evaluation, player))
    return;

  if (!name || !*name) {
    if ((thing = game_object_location(evaluation->world->database, player)) ==
        NOTHING)
      return;
  } else {
    /* Look it up */

    init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  /*
   * Check for the /debug switch
   */

  if (key == EXAM_DEBUG) {
    debug_examine(evaluation, player, thing);
    return;
  }

  buf2 = unparse_object(evaluation->world->database, evaluation, player, thing);
  examine_notify_markup(evaluation, player, nullptr, buf2);
  free_lbuf(buf2);
  notify_printf(
      evaluation, player, "Type: %s",
      object_type_entry(typeof_obj(evaluation->world->database, thing))->name);
  buf2 = flags_description(evaluation->world->database, player, thing);
  notify_checked(evaluation, player, player, buf2, MSG_ME_ALL | MSG_F_DOWN);
  free_mbuf(buf2);

  buf2 = power_description(evaluation->world->database, player, thing);
  notify_checked(evaluation, player, player, buf2, MSG_ME_ALL | MSG_F_DOWN);
  free_mbuf(buf2);
  examine_native_attributes(evaluation, player, thing);
  buf2 = unparse_object(evaluation->world->database, evaluation, player,
                        game_object_zone(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Zone: %s", buf2);
  free_lbuf(buf2);
  lua_examine_object(invocation->context->runtime->lua_owner->runtime,
                     evaluation, player, thing);
  if (!(key & EXAM_BRIEF))
    state_examine_namespaces(evaluation, player, thing);
  /*
   * show him interesting stuff
   */

  /*
   * Contents
   */

  if (game_object_contents(evaluation->world->database, thing) != NOTHING) {
    notify_checked(evaluation, player, player,
                   "Contents:", MSG_ME_ALL | MSG_F_DOWN);
    DOLIST(evaluation->world->database, content,
           game_object_contents(evaluation->world->database, thing)) {
      buf2 = unparse_object(evaluation->world->database, evaluation, player,
                            content);
      examine_notify_indented(evaluation, player, buf2);
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
      notify_checked(evaluation, player, player,
                     "Exits:", MSG_ME_ALL | MSG_F_DOWN);
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              exit);
        examine_notify_indented(evaluation, player, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify_checked(evaluation, player, player, "No exits.",
                     MSG_ME_ALL | MSG_F_DOWN);
    }

    /*
     * print dropto if present
     */

    if (game_object_location(evaluation->world->database, thing) != NOTHING) {
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_location(evaluation->world->database, thing));
      notify_printf(evaluation, player, "Dropped objects go to: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case OBJECT_TYPE_THING:
  case OBJECT_TYPE_PLAYER:

    /*
     * tell him about exits
     */

    if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
      notify_checked(evaluation, player, player,
                     "Exits:", MSG_ME_ALL | MSG_F_DOWN);
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              exit);
        examine_notify_indented(evaluation, player, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify_checked(evaluation, player, player, "No exits.",
                     MSG_ME_ALL | MSG_F_DOWN);
    }

    /*
     * print home
     */

    loc = game_object_link(evaluation->world->database, thing);
    buf2 = unparse_object(evaluation->world->database, evaluation, player, loc);
    notify_printf(evaluation, player, "Home: %s", buf2);
    free_lbuf(buf2);

    /*
     * print location if player can link to it
     */

    loc = game_object_location(evaluation->world->database, thing);
    if (loc != NOTHING) {
      buf2 =
          unparse_object(evaluation->world->database, evaluation, player, loc);
      notify_printf(evaluation, player, "Location: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case OBJECT_TYPE_EXIT:
    buf2 =
        unparse_object(evaluation->world->database, evaluation, player,
                       game_object_exits(evaluation->world->database, thing));
    notify_printf(evaluation, player, "Source: %s", buf2);
    free_lbuf(buf2);

    /*
     * print destination
     */

    switch (game_object_location(evaluation->world->database, thing)) {
    case NOTHING:
      break;
    case HOME:
      notify_checked(evaluation, player, player, "Destination: *HOME*",
                     MSG_ME_ALL | MSG_F_DOWN);
      break;
    default:
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_location(evaluation->world->database, thing));
      notify_printf(evaluation, player, "Destination: %s", buf2);
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
  const DbRef player = invocation->player;
  DbRef thing;
  char *buff, *s, *e;

  thing = game_object_contents(evaluation->world->database, player);
  if (thing == NOTHING) {
    notify_checked(evaluation, player, player, "You aren't carrying anything.",
                   MSG_ME_ALL | MSG_F_DOWN);
  } else {
    notify_checked(evaluation, player, player,
                   "You are carrying:", MSG_ME_ALL | MSG_F_DOWN);
    DOLIST(evaluation->world->database, thing, thing) {
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing);
      notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
      free_lbuf(buff);
    }
  }

  thing = game_object_exits(evaluation->world->database, player);
  if (thing != NOTHING) {
    notify_checked(evaluation, player, player,
                   "Exits:", MSG_ME_ALL | MSG_F_DOWN);
    e = buff = alloc_lbuf("look_exits");
    DOLIST(evaluation->world->database, thing, thing) {
      /*
       * chop off first exit alias to display
       */
      s = game_object_name(evaluation->world->database, thing);
      const size_t name_length = strlen(s);

      for (size_t index = 0; index < name_length; index++) {
        const char character = *(const char *)checked_storage_at_const(
            s, name_length + 1, sizeof(char), index);

        if (character == ';')
          break;
        safe_chr(character, buff, &e);
      }
      safe_str("  ", buff, &e);
    }
    *e = 0;
    notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
    free_lbuf(buff);
  }
}

void do_entrances(CommandInvocation *invocation) {
  WorldContext *world = invocation->context->world;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  char *name = invocation->first;
  DbRef thing, i;
  char *exit, *message;
  int control_thing, count;
  long low_bound, high_bound;

  parse_range(world->database, world->configuration, &name, &low_bound,
              &high_bound);
  if (!name || !*name) {
    if (has_location(evaluation->world->database, player))
      thing = game_object_location(evaluation->world->database, player);
    else
      thing = player;
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  } else {
    init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  message = alloc_lbuf("do_entrances");
  control_thing = is_examinable(evaluation->world->database, player, thing);
  count = 0;
  for (i = low_bound; i <= high_bound; i++) {
    if (control_thing ||
        is_examinable(evaluation->world->database, player, i)) {
      switch (typeof_obj(evaluation->world->database, i)) {
      case OBJECT_TYPE_EXIT:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit =
              unparse_object(evaluation->world->database, evaluation, player,
                             game_object_exits(evaluation->world->database, i));
          notify_printf(evaluation, player, "%s (%s)", exit,
                        game_object_name(evaluation->world->database, i));
          free_lbuf(exit);
          count++;
        }
        break;
      case OBJECT_TYPE_ROOM:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i);
          notify_printf(evaluation, player, "%s [dropto]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      case OBJECT_TYPE_THING:
      case OBJECT_TYPE_PLAYER:
        if (game_object_link(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i);
          notify_printf(evaluation, player, "%s [home]", exit);
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
  notify_printf(evaluation, player, "%d entrance%s found.", count,
                (count == 1) ? "" : "s");
}

/*
 * check the current location for bugs
 */
