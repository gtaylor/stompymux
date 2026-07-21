/*
 * look.c -- commands which look at things
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/commands/command_runtime.h"
#include "mux/commands/look.h"
#include "mux/commands/verbs.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/walkdb.h"
#include "mux/world/world_context.h"

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

static void look_exits(EvaluationContext *evaluation, DbRef player, DbRef loc,
                       const char *exit_name) {
  WorldContext *world = evaluation->world;
  DbRef thing;
  char *buff, *e, *s, *buff1, *e1;
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

  notify(evaluation, player, exit_name);
  e = buff = alloc_lbuf("look_exits");
  e1 = buff1 = alloc_lbuf("look_exits2");
  if (is_transparent(evaluation->world->database, loc)) {
    DOLIST(evaluation->world->database, thing,
           game_object_exits(evaluation->world->database, loc)) {
      if (exit_displayable(world->database, thing, player, key)) {
        StringCopy(buff, game_object_name(evaluation->world->database, thing));
        for (e = buff; *e && (*e != ';'); e++)
          ;
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
        e1 = buff1;
        if (buff != e)
          safe_str("  ", buff, &e);
        for (s = game_object_name(evaluation->world->database, thing);
             *s && (*s != ';'); s++)
          safe_chr(*s, buff1, &e1);
        *e1 = 0;
        safe_str(buff1, buff, &e);
      }
    }
  }

  if (!(is_transparent(evaluation->world->database, loc))) {
    safe_str("\r\n", buff, &e);
    *e = 0;
    notify(evaluation, player, buff);
  }
  free_lbuf(buff);
  free_lbuf(buff1);
}

#define CONTENTS_LOCAL 0
#define CONTENTS_NESTED 1

static void look_contents(EvaluationContext *evaluation, DbRef player,
                          DbRef loc, const char *contents_name, int style) {
  WorldContext *world = evaluation->world;
  DbRef thing;
  int can_see_loc;
  char *buff;

  /*
   * check to see if he can see the location
   */

  can_see_loc = (!is_dark(evaluation->world->database, loc) ||
                 (world->configuration->see_own_dark &&
                  is_examinable(evaluation, player, loc)));

  /*
   * check to see if there is anything there
   */

  DOLIST(evaluation->world->database, thing,
         game_object_contents(evaluation->world->database, loc)) {
    if (can_see(evaluation, world->configuration, player, thing, can_see_loc)) {

      /*
       * something exists!  show him everything
       */

      notify(evaluation, player, contents_name);
      DOLIST(evaluation->world->database, thing,
             game_object_contents(evaluation->world->database, loc)) {
        if (can_see(evaluation, world->configuration, player, thing,
                    can_see_loc)) {
          buff = unparse_object(evaluation->world->database, evaluation, player,
                                thing, 1);
          notify(evaluation, player, buff);
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
    notify(evaluation, player, result.rendered);
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

  if (is_examinable(evaluation, player, thing)) {
    buff = unparse_object(evaluation->world->database, evaluation, player,
                          thing, 1);
    notify(evaluation, player, buff);
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
  WorldContext *world = evaluation->world;
  int indent = 0;

  indent = (is_room(evaluation->world->database, loc) &&
            world->configuration->indent_desc &&
            attribute_get_raw(evaluation->world->database, loc, A_DESC));

  if (indent)
    raw_notify_newline(evaluation, player);
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
  if (indent)
    raw_notify_newline(evaluation, player);
}

static void show_desc(EvaluationContext *evaluation, DbRef player, DbRef loc,
                      int use_idesc) {
  char *got;
  DbRef aowner;
  long aflags;

  if ((typeof_obj(evaluation->world->database, loc) != TYPE_ROOM) &&
      use_idesc) {
    if (*(got = attribute_get(evaluation->world->database, loc, A_IDESC,
                              &aowner, &aflags)))
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
    buff =
        unparse_object(evaluation->world->database, evaluation, player, loc, 1);
    notify(evaluation, player, buff);
    free_lbuf(buff);

    show_desc(evaluation, player, loc,
              loc == game_object_location(evaluation->world->database, player));
  }

  /*
   * tell him the appropriate messages if he has the key
   */

  if (typeof_obj(evaluation->world->database, loc) == TYPE_ROOM) {
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
        if ((typeof_obj(evaluation->world->database, thing) == TYPE_ROOM) ||
            is_opaque(evaluation->world->database, thing)) {
          notify_quiet(evaluation, player, "You can't look outside.");
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
  init_match(&invocation->context->match, thing, name, NOTYPE);
  match_exit(&invocation->context->match);
  match_neighbor(&invocation->context->match);
  match_possession(&invocation->context->match);
  if (is_long_fingers(evaluation->world->database, player)) {
    match_absolute(&invocation->context->match);
    match_player(&invocation->context->match);
  }
  match_here(&invocation->context->match);
  match_me(&invocation->context->match);
  match_master_exit(&invocation->context->match);
  thing = match_result(&invocation->context->match);

  /*
   * Not found locally, check possessive
   */

  if (!is_good_obj(evaluation->world->database, thing)) {
    thing = match_status(evaluation, player,
                         match_possessed(&invocation->context->match, player,
                                         ((key & LOOK_OUTSIDE) ? loc : player),
                                         (char *)name, thing, 0));
  }
  /*
   * If we found something, go handle it
   */

  if (is_good_obj(evaluation->world->database, thing)) {
    switch (typeof_obj(evaluation->world->database, thing)) {
    case TYPE_ROOM:
      look_in(evaluation, player, thing, look_key);
      break;
    case TYPE_THING:
    case TYPE_PLAYER:
      if (!look_simple(evaluation, player, thing) &&
          !is_opaque(evaluation->world->database, thing)) {
        look_contents(evaluation, player, thing, "Carrying:", CONTENTS_NESTED);
      }
      break;
    case TYPE_EXIT:
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

static void debug_examine(EvaluationContext *evaluation, DbRef player,
                          DbRef thing) {
  char *buf;
  char *cp;

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
  notify_printf(evaluation, player, "Owner   = %ld",
                game_object_owner(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Zone    = %ld",
                game_object_zone(evaluation->world->database, thing));
  buf = flag_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Flags   = %s", buf);
  free_mbuf(buf);
  buf = power_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Powers  = %s", buf);
  free_mbuf(buf);
  buf = alloc_lbuf("debug_dexamine");
  cp = buf;
  safe_str("Attr list: ", buf, &cp);

  GameObject *object = game_database_object(evaluation->world->database, thing);
  for (int index = 0; index < object->at_count; index++) {
    safe_str(object->ahead[index].name, buf, &cp);
    safe_chr(' ', buf, &cp);
  }
  *cp = '\0';
  notify(evaluation, player, buf);
  free_lbuf(buf);

  for (int index = 0; index < object->at_count; index++)
    notify_printf(evaluation, player, "%s: %s", object->ahead[index].name,
                  object->ahead[index].data);
}

void do_examine(CommandInvocation *invocation) {
  WorldContext *world = invocation->context->world;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *name = invocation->first;
  DbRef thing, content, exit, aowner, loc;
  char *temp, *buf2;
  long aflags;

  /*
   * This command is pointless if the player can't hear.
   */

  if (!is_hearer(evaluation, player))
    return;

  thing = NOTHING;
  if (!name || !*name) {
    if ((thing = game_object_location(evaluation->world->database, player)) ==
        NOTHING)
      return;
  } else {
    char *pattern = strchr(name, '/');
    if (pattern) {
      *pattern++ = '\0';
      thing = match_controlled(&invocation->context->match, player, name);
      if (!is_good_obj(evaluation->world->database, thing)) {
        notify_quiet(evaluation, player, "No match.");
        return;
      }
      GameObject *object =
          game_database_object(evaluation->world->database, thing);
      bool found = false;
      for (int index = 0; index < object->at_count; index++) {
        if (!quick_wild(pattern, object->ahead[index].name))
          continue;
        notify_printf(evaluation, player, "%s: %s", object->ahead[index].name,
                      object->ahead[index].data);
        found = true;
      }
      if (!found)
        notify_quiet(evaluation, player, "No matching attributes found.");
      return;
    }

    /* Look it up */

    init_match(&invocation->context->match, player, name, NOTYPE);
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

  buf2 =
      unparse_object(evaluation->world->database, evaluation, player, thing, 0);
  notify(evaluation, player, buf2);
  free_lbuf(buf2);
  if (world->configuration->ex_flags) {
    buf2 = flag_description(evaluation->world->database, player, thing);
    notify(evaluation, player, buf2);
    free_mbuf(buf2);
  }

  temp = alloc_lbuf("do_examine.info");
  temp = attribute_get_string(evaluation->world->database, temp, thing, A_DESC,
                              &aowner, &aflags);
  if (*temp) {
    notify_printf(evaluation, player, "Desc: %s", temp);
  }

  notify_printf(
      evaluation, player, "Owner: %s",
      game_object_name(evaluation->world->database,
                       game_object_owner(evaluation->world->database, thing)));

  if (world->configuration->have_zones) {
    buf2 =
        unparse_object(evaluation->world->database, evaluation, player,
                       game_object_zone(evaluation->world->database, thing), 0);
    notify_printf(evaluation, player, "Zone: %s", buf2);
    free_lbuf(buf2);
  }
  lua_examine_object(invocation->context->runtime->lua_owner->runtime,
                     evaluation, player, thing);
  buf2 = power_description(evaluation->world->database, player, thing);
  notify(evaluation, player, buf2);
  free_mbuf(buf2);
  if (key != EXAM_BRIEF) {
    GameObject *object =
        game_database_object(evaluation->world->database, thing);
    for (int index = 0; index < object->at_count; index++)
      notify_printf(evaluation, player, "%s: %s", object->ahead[index].name,
                    object->ahead[index].data);
  }

  /*
   * show him interesting stuff
   */

  /*
   * Contents
   */

  if (game_object_contents(evaluation->world->database, thing) != NOTHING) {
    notify(evaluation, player, "Contents:");
    DOLIST(evaluation->world->database, content,
           game_object_contents(evaluation->world->database, thing)) {
      buf2 = unparse_object(evaluation->world->database, evaluation, player,
                            content, 0);
      notify(evaluation, player, buf2);
      free_lbuf(buf2);
    }
  }
  /*
   * Show stuff that depends on the object type
   */

  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_ROOM:

    /*
     * tell him about exits
     */

    if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
      notify(evaluation, player, "Exits:");
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              exit, 0);
        notify(evaluation, player, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify(evaluation, player, "No exits.");
    }

    /*
     * print dropto if present
     */

    if (game_object_location(evaluation->world->database, thing) != NOTHING) {
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_location(evaluation->world->database, thing), 0);
      notify_printf(evaluation, player, "Dropped objects go to: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case TYPE_THING:
  case TYPE_PLAYER:

    /*
     * tell him about exits
     */

    if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
      notify(evaluation, player, "Exits:");
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              exit, 0);
        notify(evaluation, player, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify(evaluation, player, "No exits.");
    }

    /*
     * print home
     */

    loc = game_object_link(evaluation->world->database, thing);
    buf2 =
        unparse_object(evaluation->world->database, evaluation, player, loc, 0);
    notify_printf(evaluation, player, "Home: %s", buf2);
    free_lbuf(buf2);

    /*
     * print location if player can link to it
     */

    loc = game_object_location(evaluation->world->database, thing);
    if (loc != NOTHING) {
      buf2 = unparse_object(evaluation->world->database, evaluation, player,
                            loc, 0);
      notify_printf(evaluation, player, "Location: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case TYPE_EXIT:
    buf2 = unparse_object(evaluation->world->database, evaluation, player,
                          game_object_exits(evaluation->world->database, thing),
                          0);
    notify_printf(evaluation, player, "Source: %s", buf2);
    free_lbuf(buf2);

    /*
     * print destination
     */

    switch (game_object_location(evaluation->world->database, thing)) {
    case NOTHING:
      break;
    case HOME:
      notify(evaluation, player, "Destination: *HOME*");
      break;
    default:
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_location(evaluation->world->database, thing), 0);
      notify_printf(evaluation, player, "Destination: %s", buf2);
      free_lbuf(buf2);
      break;
    }
    break;
  default:
    break;
  }
  free_lbuf(temp);
}

void do_inventory(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  DbRef thing;
  char *buff, *s, *e;

  thing = game_object_contents(evaluation->world->database, player);
  if (thing == NOTHING) {
    notify(evaluation, player, "You aren't carrying anything.");
  } else {
    notify(evaluation, player, "You are carrying:");
    DOLIST(evaluation->world->database, thing, thing) {
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing, 1);
      notify(evaluation, player, buff);
      free_lbuf(buff);
    }
  }

  thing = game_object_exits(evaluation->world->database, player);
  if (thing != NOTHING) {
    notify(evaluation, player, "Exits:");
    e = buff = alloc_lbuf("look_exits");
    DOLIST(evaluation->world->database, thing, thing) {
      /*
       * chop off first exit alias to display
       */
      for (s = game_object_name(evaluation->world->database, thing);
           *s && (*s != ';'); s++)
        safe_chr(*s, buff, &e);
      safe_str("  ", buff, &e);
    }
    *e = 0;
    notify(evaluation, player, buff);
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
    init_match(&invocation->context->match, player, name, NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  message = alloc_lbuf("do_entrances");
  control_thing = is_examinable(evaluation, player, thing);
  count = 0;
  for (i = low_bound; i <= high_bound; i++) {
    if (control_thing || is_examinable(evaluation, player, i)) {
      switch (typeof_obj(evaluation->world->database, i)) {
      case TYPE_EXIT:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit = unparse_object(
              evaluation->world->database, evaluation, player,
              game_object_exits(evaluation->world->database, i), 0);
          notify_printf(evaluation, player, "%s (%s)", exit,
                        game_object_name(evaluation->world->database, i));
          free_lbuf(exit);
          count++;
        }
        break;
      case TYPE_ROOM:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i, 0);
          notify_printf(evaluation, player, "%s [dropto]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      case TYPE_THING:
      case TYPE_PLAYER:
        if (game_object_link(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i, 0);
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
