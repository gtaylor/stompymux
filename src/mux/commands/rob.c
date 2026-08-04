/*
 * rob.c -- Commands dealing with giving and taking things
 */

#include "mux/commands/action_messages.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_keys.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/move.h"

/*
 * ---------------------------------------------------------------------------
 * * give_thing, do_give: Give away things.
 */

static void give_thing(EvaluationContext *evaluation, DbRef giver,
                       DbRef recipient, int key, char *what) {
  MatchContext *match = &evaluation->command->match;
  DbRef thing;
  char *str, *sp;
  LuaLockInvocation lock;
  LuaLockResult result;

  init_match(match, giver, what, OBJECT_TYPE_THING);
  match_possession(match);
  match_me(match);
  thing = match_result(match);

  switch (thing) {
  case NOTHING:
    notify_checked(evaluation, giver, giver, "You don't have that!",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  case AMBIGUOUS:
    notify_checked(evaluation, giver, giver, "I don't know which you mean!",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  default:
    break;
  }

  if (thing == giver) {
    notify_checked(evaluation, giver, giver, "You can't give yourself away!",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  if ((typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_THING) &&
      (typeof_obj(evaluation->world->database, thing) != OBJECT_TYPE_PLAYER)) {
    notify_checked(evaluation, giver, giver, "Permission denied.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  if (!lock_test(evaluation, giver, giver, giver, thing, LUA_LOCK_GIVE,
                 LUA_LOCK_OPERATION_GIVE, false, &lock, &result)) {
    sp = str = alloc_lbuf("do_give.gfail");
    safe_str("You can't give ", str, &sp);
    safe_str(game_object_name(evaluation->world->database, thing), str, &sp);
    safe_str(" away.", str, &sp);
    *sp = '\0';

    notify_lock_failure(evaluation, &lock, &result, str, nullptr,
                        LUA_EVENT_GIVE_FAIL);
    free_lbuf(str);
    return;
  }
  if (!lock_test(evaluation, giver, giver, thing, recipient, LUA_LOCK_RECEIVE,
                 LUA_LOCK_OPERATION_RECEIVE, false, &lock, &result)) {
    sp = str = alloc_lbuf("do_give.rfail");
    safe_str(game_object_name(evaluation->world->database, recipient), str,
             &sp);
    safe_str(" doesn't want ", str, &sp);
    safe_str(game_object_name(evaluation->world->database, thing), str, &sp);
    safe_chr('.', str, &sp);
    *sp = '\0';

    notify_lock_failure(evaluation, &lock, &result, str, nullptr,
                        LUA_EVENT_GIVE_RECEIVE_FAIL);
    free_lbuf(str);
    return;
  }
  move_via_generic(evaluation, thing, recipient, giver, 0);
  if (!(key & GIVE_QUIET)) {
    str = alloc_lbuf("do_give.thing.ok");
    StringCopy(str, game_object_name(evaluation->world->database, giver));
    notify_checked(
        evaluation, recipient, giver,
        tprintf("%s gave you %s.", str,
                game_object_name(evaluation->world->database, thing)),
        MSG_ME_ALL | MSG_F_DOWN);
    notify_checked(evaluation, giver, giver, "Given.", MSG_ME_ALL | MSG_F_DOWN);
    notify_checked(
        evaluation, thing, giver,
        tprintf("%s gave you to %s.", str,
                game_object_name(evaluation->world->database, recipient)),
        MSG_ME_ALL | MSG_F_DOWN);
    free_lbuf(str);
  }
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_DROP,
                                .operation = LUA_MESSAGE_OPERATION_GIVE,
                                .object = thing,
                                .enactor = giver,
                                .cause = giver,
                                .source = NOTHING,
                                .destination = NOTHING},
                    .event = LUA_EVENT_DROP});
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_SUCCESS,
                                .operation = LUA_MESSAGE_OPERATION_RECEIVE,
                                .object = thing,
                                .enactor = recipient,
                                .cause = giver,
                                .source = NOTHING,
                                .destination = NOTHING},
                    .event = LUA_EVENT_SUCCESS});
}

void do_give(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *who = invocation->first;
  char *amnt = invocation->second;
  DbRef recipient;
  MatchContext *match = &invocation->context->match;

  /*
   * check recipient
   */

  init_match(match, player, who, OBJECT_TYPE_PLAYER);
  match_neighbor(match);
  match_possession(match);
  match_me(match);
  recipient = match_result(match);
  switch (recipient) {
  case NOTHING:
    notify_checked(evaluation, player, player, "Give to whom?",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  case AMBIGUOUS:
    notify_checked(evaluation, player, player, "I don't know who you mean!",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  default:
    break;
  }

  give_thing(&invocation->context->evaluation, player, recipient, key, amnt);
}
