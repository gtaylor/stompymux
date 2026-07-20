/*
 * rob.c -- Commands dealing with giving and taking things
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"
#include "mux/world/world_context.h"

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

  init_match(match, giver, what, TYPE_THING);
  match_possession(match);
  match_me(match);
  thing = match_result(match);

  switch (thing) {
  case NOTHING:
    notify(evaluation, giver, "You don't have that!");
    return;
  case AMBIGUOUS:
    notify(evaluation, giver, "I don't know which you mean!");
    return;
  default:
    break;
  }

  if (thing == giver) {
    notify(evaluation, giver, "You can't give yourself away!");
    return;
  }
  if (((typeof_obj(evaluation->world->database, thing) != TYPE_THING) &&
       (typeof_obj(evaluation->world->database, thing) != TYPE_PLAYER)) ||
      !(is_enter_ok(evaluation->world->database, recipient) ||
        is_controls(evaluation, giver, recipient))) {
    notify(evaluation, giver, "Permission denied.");
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
  divest_object(evaluation, thing);
  if (!(key & GIVE_QUIET)) {
    str = alloc_lbuf("do_give.thing.ok");
    StringCopy(str, game_object_name(evaluation->world->database, giver));
    notify_with_cause(
        evaluation, recipient, giver,
        tprintf("%s gave you %s.", str,
                game_object_name(evaluation->world->database, thing)));
    notify(evaluation, giver, "Given.");
    notify_with_cause(
        evaluation, thing, giver,
        tprintf("%s gave you to %s.", str,
                game_object_name(evaluation->world->database, recipient)));
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

  init_match(match, player, who, TYPE_PLAYER);
  match_neighbor(match);
  match_possession(match);
  match_me(match);
  if (is_long_fingers(evaluation->world->database, player)) {
    match_player(match);
    match_absolute(match);
  }
  recipient = match_result(match);
  switch (recipient) {
  case NOTHING:
    notify(evaluation, player, "Give to whom?");
    return;
  case AMBIGUOUS:
    notify(evaluation, player, "I don't know who you mean!");
    return;
  default:
    break;
  }

  give_thing(&invocation->context->evaluation, player, recipient, key, amnt);
}
