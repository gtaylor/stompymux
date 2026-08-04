/*
 * object_set.c -- Commands that manipulate object properties and attributes
 */

#include "mux/world/object_set.h"

#include <stdio.h>
#include <string.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_keys.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/validation.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/player.h"
#include "p.glue.h"

DbRef match_controlled(MatchContext *match, DbRef player, char *name) {
  DbRef mat;

  init_match(match, player, name, OBJECT_TYPE_NOTYPE);
  match_everything(match, 0);
  mat = noisy_match_result(match);
  if (is_good_obj(match->evaluation->world->database, mat) &&
      !is_controls(match->evaluation->world->database, player, mat)) {
    notify_checked(match->evaluation, player, player, "Permission denied.",
                   MSG_ME);
    return NOTHING;
  } else {
    return (mat);
  }
}

DbRef match_controlled_quiet(MatchContext *match, DbRef player, char *name) {
  DbRef mat;

  init_match(match, player, name, OBJECT_TYPE_NOTYPE);
  match_everything(match, 0);
  mat = match_result(match);
  if (is_good_obj(match->evaluation->world->database, mat) &&
      !is_controls(match->evaluation->world->database, player, mat)) {
    return NOTHING;
  } else {
    return (mat);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_alias: Make an alias for a player.
 */

void do_alias(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *alias = invocation->second;
  DbRef thing;
  long aflags;
  char *oldalias, *trimalias;

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;

  /*
   * check for renaming a player
   */

  if (is_player(evaluation->world->database, thing)) {

    /*
     * Fetch the old alias
     */

    oldalias =
        attribute_get(evaluation->world->database, thing, A_ALIAS, &aflags);
    trimalias = trim_spaces(alias);

    if (!is_controls(evaluation->world->database, player, thing)) {

      /*
       * Make sure we have rights to do it.  We can't do *
       * * * * the normal Set_attr check because ALIAS is *
       * only * * * writable by GOD and we want to keep *
       * people * from * * doing &ALIAS and bypassing the *
       * player * name checks.
       */

      notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
    } else if (!*trimalias) {

      /*
       * New alias is null, just clear it
       */

      delete_player_name(invocation->context->world, thing, oldalias);
      attribute_clear(evaluation->world->database, thing, A_ALIAS);
      if (!is_quiet(evaluation->world->database, player))
        notify_checked(evaluation, player, player, "Alias removed.", MSG_ME);
    } else if (lookup_player(invocation->context->world, NOTHING, trimalias,
                             0) != NOTHING) {

      /*
       * Make sure new alias isn't already in use
       */

      notify_checked(evaluation, player, player, "That name is already in use.",
                     MSG_ME);
    } else if (!(badname_check(invocation->context->world, trimalias) &&
                 ok_player_name(invocation->context->world->configuration,
                                trimalias))) {
      notify_checked(evaluation, player, player,
                     "That's a silly name for a player!", MSG_ME);
    } else {

      /*
       * Remove the old name and add the new name
       */

      delete_player_name(invocation->context->world, thing, oldalias);
      attribute_add(evaluation->world->database, thing, A_ALIAS, trimalias,
                    aflags);
      if (add_player_name(invocation->context->world, thing, trimalias)) {
        if (!is_quiet(evaluation->world->database, player))
          notify_checked(evaluation, player, player, "Alias set.", MSG_ME);
      } else {
        notify_checked(
            evaluation, player, player,
            "That name is already in use or is illegal, alias cleared.",
            MSG_ME);
        attribute_clear(evaluation->world->database, thing, A_ALIAS);
      }
    }
    free_lbuf(trimalias);
    free_lbuf(oldalias);
  } else {
    notify_checked(evaluation, player, player, "Only players may have aliases.",
                   MSG_ME);
  }
}

void object_attribute_set(EvaluationContext *evaluation, DbRef player,
                          DbRef thing, int attrnum, char *attrtext, int key) {
  long aflags;
  int have_xcode;
  Attribute *attr;
  char *compiled = nullptr;
  char error[256];

  attr = attribute_by_number(evaluation->world->database, attrnum);
  attribute_get_info(evaluation->world->database, thing, attrnum, &aflags);
  if (attr && set_attr(evaluation, player, thing, attr, aflags)) {
    if (attrnum == A_ALIAS &&
        (!is_player(evaluation->world->database, thing) ||
         (*attrtext &&
          !ok_player_name(evaluation->world->configuration, attrtext)))) {
      notify_checked(evaluation, player, player,
                     "Player aliases must use valid printable ASCII names.",
                     MSG_ME);
      return;
    }
    if (attrnum == A_DESC || attrnum == A_IDESC) {
      compiled = alloc_lbuf("object_attribute_set.style");
      if (!styled_text_compile(evaluation->world->styled_text_palette, attrtext,
                               compiled, LBUF_SIZE, error, sizeof(error))) {
        notify_printf(evaluation, player, "Invalid styled-text markup: %s.",
                      error);
        free_lbuf(compiled);
        return;
      }
    }
    have_xcode = is_xcode(evaluation->world->database, thing);
    attribute_add(evaluation->world->database, thing, attrnum, attrtext,
                  aflags);
    handle_xcode(evaluation->btech, player, thing, have_xcode,
                 is_xcode(evaluation->world->database, thing));
    if (!(key & SET_QUIET) && !is_quiet(evaluation->world->database, player) &&
        !is_quiet(evaluation->world->database, thing))
      notify_printf(evaluation, player, "%s/%s - %s",
                    game_object_name(evaluation->world->database, thing),
                    attr->name, strlen(attrtext) ? "Set." : "Cleared.");
    free_lbuf(compiled);
  } else {
    notify_checked(evaluation, player, player, "Permission denied.", MSG_ME);
  }
}

void do_power(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *flag = invocation->second;
  DbRef thing;

  if (!flag || !*flag) {
    notify_checked(evaluation, player, player,
                   "I don't know what you want to set!", MSG_ME);
    return;
  }
  /*
   * find thing
   */

  if ((thing = match_controlled(&invocation->context->match, player, name)) ==
      NOTHING)
    return;

  power_set(&invocation->context->evaluation,
            invocation->context->runtime->world_indexes, thing, player, flag,
            invocation->key);
}

void do_setattr(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  int attrnum = invocation->key;
  char *name = invocation->first;
  char *attrtext = invocation->second;
  DbRef thing;

  init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
  match_everything(&invocation->context->match, 0);
  thing = noisy_match_result(&invocation->context->match);

  if (thing == NOTHING)
    return;
  object_attribute_set(&invocation->context->evaluation, player, thing, attrnum,
                       attrtext, 0);
}

void do_use(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  char *object = invocation->first;
  char *df_use, *df_ouse;
  DbRef thing;
  int doit;
  LuaLockInvocation lock;
  LuaLockResult result;

  init_match(&invocation->context->match, player, object, OBJECT_TYPE_NOTYPE);
  match_neighbor(&invocation->context->match);
  match_possession(&invocation->context->match);
  if (is_wizard(evaluation->world->database, player)) {
    match_absolute(&invocation->context->match);
    match_player(&invocation->context->match);
  }
  match_me(&invocation->context->match);
  match_here(&invocation->context->match);
  thing = noisy_match_result(&invocation->context->match);
  if (thing == NOTHING)
    return;

  /*
   * Make sure player can use it
   */

  if (!lock_test(evaluation, player, invocation->cause, player, thing,
                 LUA_LOCK_USE, LUA_LOCK_OPERATION_USE, false, &lock, &result)) {
    notify_lock_failure(evaluation, &lock, &result,
                        "You can't figure out how to use that.", nullptr,
                        LUA_EVENT_USE_FAIL);
    return;
  }
  doit = 0;
  if (lua_message_defined(evaluation->runtime->lua_owner->runtime, thing,
                          LUA_MESSAGE_USE) ||
      lua_event_defined(evaluation->runtime->lua_owner->runtime, thing,
                        LUA_EVENT_USE))
    doit = 1;
  if (doit) {
    df_use = alloc_lbuf("do_use.use");
    df_ouse = alloc_lbuf("do_use.ouse");
    snprintf(df_use, LBUF_SIZE, "You use %s",
             game_object_name(evaluation->world->database, thing));
    snprintf(df_ouse, LBUF_SIZE, "uses %s",
             game_object_name(evaluation->world->database, thing));
    notify_action(&invocation->context->evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_USE,
                                  .operation = LUA_MESSAGE_OPERATION_USE,
                                  .descriptor = invocation->context->descriptor,
                                  .object = thing,
                                  .enactor = player,
                                  .cause = invocation->cause,
                                  .source = NOTHING,
                                  .destination = NOTHING},
                      .enactor_default = df_use,
                      .other_default = df_ouse,
                      .event = LUA_EVENT_USE});
    free_lbuf(df_use);
    free_lbuf(df_ouse);
  } else {
    notify_checked(evaluation, player, player,
                   "You can't figure out how to use that.", MSG_ME);
  }
}
