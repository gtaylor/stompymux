/* action_messages.c - Action messaging and native Lua event dispatch. */

#include "mux/commands/action_messages.h"
#include <stdio.h>

#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/world/object_spatial.h"

void notify_action(EvaluationContext *evaluation,
                   const ActionMessageInvocation *invocation) {
  char message_buffer[LBUF_SIZE];
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
  } else if (enactor_message && *enactor_message) {
    notify_checked(evaluation, message.enactor, message.enactor,
                   enactor_message, MSG_ME_ALL | MSG_F_DOWN);
  }
  /*
   * message to neighbors
   */

  bool notify_location = false;
  if (!message.silent && other_message && *other_message &&
      has_location(evaluation->world->database, message.enactor)) {
    location =
        game_object_location(evaluation->world->database, message.enactor);
    notify_location = is_good_obj(evaluation->world->database, location);
  }
  if (notify_location) {
    (void)snprintf(
        message_buffer, sizeof(message_buffer), "%s %s",
        game_object_name(evaluation->world->database, message.enactor),
        other_message);
    notify_excluding(&(ExcludingNotification){
        .evaluation = evaluation,
        .location = location,
        .sender = message.enactor,
        .exceptions = {message.enactor, message.object},
        .exception_count = 2,
        .message = message_buffer});
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

void notify_lock_failure(const LockFailureNotification *notification) {
  char message_buffer[LBUF_SIZE];
  EvaluationContext *evaluation = notification->evaluation;
  const LuaLockInvocation *invocation = notification->invocation;
  const LuaLockResult *result = notification->result;
  const char *enactor_message = result->has_enactor_message
                                    ? result->enactor_message
                                    : notification->enactor_default;
  const char *other_message = result->has_other_message
                                  ? result->other_message
                                  : notification->other_default;
  DbRef location;

  if (invocation->silent)
    return;
  if (enactor_message && *enactor_message)
    notify_checked(evaluation, invocation->enactor, invocation->enactor,
                   enactor_message, MSG_ME_ALL | MSG_F_DOWN);
  bool notify_location = false;
  if (other_message && *other_message &&
      has_location(evaluation->world->database, invocation->enactor)) {
    location =
        game_object_location(evaluation->world->database, invocation->enactor);
    notify_location = is_good_obj(evaluation->world->database, location);
  }
  if (notify_location) {
    (void)snprintf(
        message_buffer, sizeof(message_buffer), "%s %s",
        game_object_name(evaluation->world->database, invocation->enactor),
        other_message);
    notify_excluding(&(ExcludingNotification){
        .evaluation = evaluation,
        .location = location,
        .sender = invocation->enactor,
        .exceptions = {invocation->enactor, invocation->object},
        .exception_count = 2,
        .message = message_buffer});
  }
  if (notification->event != LUA_EVENT_NONE) {
    LuaEventInvocation event_invocation = {
        .type = notification->event,
        .descriptor = invocation->descriptor,
        .object = invocation->object,
        .enactor = invocation->enactor,
        .cause = invocation->cause,
    };

    lua_event_dispatch(evaluation->runtime->lua_owner->runtime,
                       &event_invocation);
  }
}
