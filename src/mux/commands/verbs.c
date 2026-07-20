/* verbs.c - Action messaging and native Lua event dispatch. */

#include "mux/commands/verbs.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/world/object_spatial.h"

void notify_action(EvaluationContext *evaluation,
                   const ActionMessageInvocation *invocation) {
  LuaMessageInvocation message = invocation->message;
  LuaMessageResult result;
  const char *enactor_message;
  const char *other_message;
  char *d, *buff, *bp, *str;
  DbRef location, attribute_owner;
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
    d = attribute_parent_get(evaluation->world->database, message.object,
                             invocation->content_attribute, &attribute_owner,
                             &attribute_flags);
    if (*d) {
      buff = bp = alloc_lbuf("notify_action.1");
      str = d;
      exec(evaluation, buff, &bp, 0, message.object, message.enactor,
           EV_EVAL | EV_FIGNORE | EV_TOP, &str, invocation->arguments,
           invocation->argument_count);
      *bp = '\0';
      notify(evaluation, message.enactor, buff);
      free_lbuf(buff);
    } else if (enactor_message) {
      notify(evaluation, message.enactor, enactor_message);
    }
    free_lbuf(d);
  } else if (enactor_message && *enactor_message)
    notify(evaluation, message.enactor, enactor_message);
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
    notify(evaluation, invocation->enactor, enactor_message);
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
