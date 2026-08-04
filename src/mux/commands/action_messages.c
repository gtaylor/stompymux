/* action_messages.c - Action messaging and native Lua event dispatch. */

#include "mux/commands/action_messages.h"

#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/world/object_spatial.h"

void notify_action(EvaluationContext *evaluation,
                   const ActionMessageInvocation *invocation) {
  LuaMessageInvocation message = invocation->message;
  LuaMessageResult result;
  const char *enactor_message;
  const char *other_message;
  char *d;
  DbRef location;
  long attribute_flags;

  if (!message.descriptor && evaluation->command)
    message.descriptor = evaluation->command->descriptor;
  lua_message_evaluate(evaluation->runtime->lua_owner->runtime, &message,
                       &result);
  enactor_message = result.has_enactor_message ? result.enactor_message
                                               : invocation->enactor_default;
  other_message = result.has_other_message ? result.other_message
                                           : invocation->other_default;

  /*
   * message to player
   */

  if (invocation->content_attribute > 0) {
    d = attribute_get(evaluation->world->database, message.object,
                      invocation->content_attribute, &attribute_flags);
    if (*d) {
      notify_checked(evaluation, message.enactor, message.enactor, d,
                     MSG_ME_ALL | MSG_F_DOWN);
    } else if (enactor_message) {
      notify_checked(evaluation, message.enactor, message.enactor,
                     enactor_message, MSG_ME_ALL | MSG_F_DOWN);
    }
    free_lbuf(d);
  } else if (enactor_message && *enactor_message)
    notify_checked(evaluation, message.enactor, message.enactor,
                   enactor_message, MSG_ME_ALL | MSG_F_DOWN);
  /*
   * message to neighbors
   */

  if (!message.silent && other_message && *other_message &&
      has_location(evaluation->world->database, message.enactor) &&
      is_good_obj(evaluation->world->database,
                  location = game_object_location(evaluation->world->database,
                                                  message.enactor))) {
    notify_except2(
        evaluation, location, message.enactor, message.enactor, message.object,
        tprintf("%s %s",
                game_object_name(evaluation->world->database, message.enactor),
                other_message));
  }
  if (invocation->event != LUA_EVENT_NONE) {
    LuaEventInvocation event_invocation = {
        .type = invocation->event,
        .descriptor = message.descriptor,
        .object = message.object,
        .enactor = message.enactor,
        .cause = message.cause,
        .arguments = invocation->arguments,
        .argument_count = invocation->argument_count,
    };

    lua_event_dispatch(evaluation->runtime->lua_owner->runtime,
                       &event_invocation);
  }
}

void notify_event(EvaluationContext *evaluation, Descriptor *descriptor,
                  DbRef enactor, DbRef cause, DbRef object, LuaEventType event,
                  char **arguments, int argument_count) {
  LuaEventInvocation invocation = {
      .type = event,
      .descriptor = descriptor,
      .object = object,
      .enactor = enactor,
      .cause = cause,
      .arguments = arguments,
      .argument_count = argument_count,
  };

  lua_event_dispatch(evaluation->runtime->lua_owner->runtime, &invocation);
}

void notify_lock_failure(EvaluationContext *evaluation,
                         const LuaLockInvocation *invocation,
                         const LuaLockResult *result,
                         const char *enactor_default, const char *other_default,
                         LuaEventType event) {
  const char *enactor_message =
      result->has_enactor_message ? result->enactor_message : enactor_default;
  const char *other_message =
      result->has_other_message ? result->other_message : other_default;
  DbRef location;

  if (invocation->silent)
    return;
  if (enactor_message && *enactor_message)
    notify_checked(evaluation, invocation->enactor, invocation->enactor,
                   enactor_message, MSG_ME_ALL | MSG_F_DOWN);
  if (other_message && *other_message &&
      has_location(evaluation->world->database, invocation->enactor) &&
      is_good_obj(evaluation->world->database,
                  location = game_object_location(evaluation->world->database,
                                                  invocation->enactor))) {
    notify_except2(evaluation, location, invocation->enactor,
                   invocation->enactor, invocation->object,
                   tprintf("%s %s",
                           game_object_name(evaluation->world->database,
                                            invocation->enactor),
                           other_message));
  }
  if (event != LUA_EVENT_NONE) {
    LuaEventInvocation event_invocation = {
        .type = event,
        .descriptor = invocation->descriptor,
        .object = invocation->object,
        .enactor = invocation->enactor,
        .cause = invocation->cause,
    };

    lua_event_dispatch(evaluation->runtime->lua_owner->runtime,
                       &event_invocation);
  }
}
