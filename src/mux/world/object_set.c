/*
 * object_set.c -- Commands that manipulate object properties and attributes
 */

#include "mux/world/object_set.h"

#include <stdio.h>

#include "mux/commands/action_messages.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_keys.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/player_account.h"
#include "mux/objects/powers.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/owned_text.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/validation.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/player.h"

DbRef match_controlled(MatchContext *match, DbRef player, const char *name) {
  DbRef mat;

  init_match(match, player, name, OBJECT_TYPE_NOTYPE);
  match_everything(match, 0);
  mat = noisy_match_result(match);
  if (is_good_obj(match->evaluation->world->database, mat) &&
      !is_controls(match->evaluation->world->database, player, mat)) {
    notify_checked(match->evaluation, player, player, "Permission denied.",
                   MSG_ME);
    return NOTHING;
  }
  return (mat);
}

DbRef match_controlled_quiet(MatchContext *match, DbRef player,
                             const char *name) {
  DbRef mat;

  init_match(match, player, name, OBJECT_TYPE_NOTYPE);
  match_everything(match, 0);
  mat = match_result(match);
  if (is_good_obj(match->evaluation->world->database, mat) &&
      !is_controls(match->evaluation->world->database, player, mat)) {
    return NOTHING;
  }
  return (mat);
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
  const char *oldalias;
  OwnedText trimalias;

  thing = match_controlled(&invocation->context->match, player, name);
  if (thing == NOTHING)
    return;

  /*
   * check for renaming a player
   */

  if (is_player(evaluation->world->database, thing)) {

    /*
     * Fetch the old alias
     */

    oldalias = player_account_alias(evaluation->world->database, thing);
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
    } else if (!*trimalias.text) {

      /*
       * New alias is null, just clear it
       */

      delete_player_name(invocation->context->world, thing, oldalias);
      if (player_account_alias_set(evaluation->world->database, thing,
                                   nullptr)) {
        notify_checked(evaluation, player, player, "Alias removed.", MSG_ME);
      } else {
        if (*oldalias)
          (void)add_player_name(invocation->context->world, thing, oldalias);
        notify_checked(evaluation, player, player, "Unable to remove alias.",
                       MSG_ME);
      }
    } else if (lookup_player(invocation->context->world, NOTHING,
                             trimalias.text, 0) != NOTHING) {

      /*
       * Make sure new alias isn't already in use
       */

      notify_checked(evaluation, player, player, "That name is already in use.",
                     MSG_ME);
    } else if (!(badname_check(invocation->context->world, trimalias.text) &&
                 ok_player_name(invocation->context->world->configuration,
                                trimalias.text))) {
      notify_checked(evaluation, player, player,
                     "That's a silly name for a player!", MSG_ME);
    } else {

      /*
       * Remove the old name and add the new name
       */

      delete_player_name(invocation->context->world, thing, oldalias);
      if (!player_account_alias_set(evaluation->world->database, thing,
                                    trimalias.text)) {
        if (*oldalias)
          (void)add_player_name(invocation->context->world, thing, oldalias);
        notify_checked(evaluation, player, player, "Unable to set alias.",
                       MSG_ME);
      } else if (add_player_name(invocation->context->world, thing,
                                 trimalias.text)) {
        notify_checked(evaluation, player, player, "Alias set.", MSG_ME);
      } else {
        notify_checked(
            evaluation, player, player,
            "That name is already in use or is illegal, alias cleared.",
            MSG_ME);
        if (!player_account_alias_set(evaluation->world->database, thing,
                                      nullptr))
          notify_checked(evaluation, player, player,
                         "Unable to clear the rejected alias.", MSG_ME);
      }
    }
    owned_text_release(&trimalias);
  } else {
    notify_checked(evaluation, player, player, "Only players may have aliases.",
                   MSG_ME);
  }
}

void do_description(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef object = match_controlled(&invocation->context->match,
                                  invocation->player, invocation->first);
  char compiled[LBUF_SIZE];
  char error[256];

  if (object == NOTHING)
    return;
  if (!styled_text_compile(evaluation->world->styled_text_palette,
                           invocation->second, compiled, sizeof(compiled),
                           error, sizeof(error))) {
    notify_printf(evaluation, invocation->player,
                  "Invalid styled-text markup: %s.", error);
    return;
  }
  const bool IS_INTERNAL = invocation->key == DESCRIPTION_INTERNAL;
  if (IS_INTERNAL) {
    game_object_internal_description_set(evaluation->world->database, object,
                                         invocation->second);
  } else {
    game_object_description_set(evaluation->world->database, object,
                                invocation->second);
  }
  notify_printf(evaluation, invocation->player, "%s/%s - %s",
                game_object_name(evaluation->world->database, object),
                IS_INTERNAL ? "InternalDescription" : "Description",
                *invocation->second ? "Set." : "Cleared.");
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

  thing = match_controlled(&invocation->context->match, player, name);
  if (thing == NOTHING)
    return;

  power_set(&invocation->context->evaluation,
            invocation->context->runtime->world_indexes, thing, player, flag,
            invocation->key);
}

void do_use(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef PLAYER = invocation->player;
  char *object = invocation->first;
  char *df_use;
  char *df_ouse;
  DbRef thing;
  int doit;
  LuaLockInvocation lock;

  init_match(&invocation->context->match, PLAYER, object, OBJECT_TYPE_NOTYPE);
  match_neighbor(&invocation->context->match);
  match_possession(&invocation->context->match);
  if (is_wizard(evaluation->world->database, PLAYER)) {
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

  LuaLockResult *result = checked_storage_allocate(sizeof(*result));
  if (!lock_test(evaluation, PLAYER, invocation->cause, PLAYER, thing,
                 LUA_LOCK_USE, false, &lock, result)) {
    notify_lock_failure(&(LockFailureNotification){
        .evaluation = evaluation,
        .invocation = &lock,
        .result = result,
        .enactor_default = "You can't figure out how to use that.",
        .event = LUA_EVENT_USE_FAIL});
    free_buf(result);
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
    (void)snprintf(df_use, LBUF_SIZE, "You use %s",
                   game_object_name(evaluation->world->database, thing));
    (void)snprintf(df_ouse, LBUF_SIZE, "uses %s",
                   game_object_name(evaluation->world->database, thing));
    notify_action(&invocation->context->evaluation,
                  &(ActionMessageInvocation){
                      .message = {.type = LUA_MESSAGE_USE,
                                  .operation = LUA_MESSAGE_OPERATION_USE,
                                  .descriptor = invocation->context->descriptor,
                                  .object = thing,
                                  .enactor = PLAYER,
                                  .cause = invocation->cause,
                                  .source = NOTHING,
                                  .destination = NOTHING},
                      .enactor_default = df_use,
                      .other_default = df_ouse,
                      .event = LUA_EVENT_USE});
    free_buf(df_use);
    free_buf(df_ouse);
  } else {
    notify_checked(evaluation, PLAYER, PLAYER,
                   "You can't figure out how to use that.", MSG_ME);
  }
  free_buf(result);
}
