/** @file
 * Action messaging and native Lua event dispatch.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

typedef enum ActionMessageContent : int {
  ACTION_MESSAGE_CONTENT_NONE,
  ACTION_MESSAGE_CONTENT_DESCRIPTION,
  ACTION_MESSAGE_CONTENT_INTERNAL_DESCRIPTION,
} ActionMessageContent;

typedef struct ActionMessageInvocation {
  LuaMessageInvocation message;
  ActionMessageContent content;
  const char *enactor_default;
  const char *other_default;
  LuaEventType event;
  char **arguments;
  int argument_count;
} ActionMessageInvocation;

typedef struct LockFailureNotification {
  EvaluationContext *evaluation;
  const LuaLockInvocation *invocation;
  const LuaLockResult *result;
  const char *enactor_default;
  const char *other_default;
  LuaEventType event;
} LockFailureNotification;

/** Sends notify action. @param[in,out] evaluation Expression evaluation
 * context. @param[in] invocation Command invocation. */

void notify_action(EvaluationContext *evaluation,
                   const ActionMessageInvocation *invocation);
/** Sends notify event. @param[in,out] evaluation Expression evaluation context.
 * @param[in,out] descriptor Network descriptor. @param[in] enactor Object that
 * initiated the operation. @param[in] cause Object that caused the operation.
 * @param[in] object Game object. @param[in] event Event. @param[in,out]
 * arguments Argument list. @param[in] argument_count Number of argument
 * entries. */

void notify_event(EvaluationContext *evaluation, Descriptor *descriptor,
                  DbRef enactor, DbRef cause, DbRef object, LuaEventType event,
                  char **arguments, int argument_count);
/** Sends notify lock failure. @param[in] notification Notification. */

void notify_lock_failure(const LockFailureNotification *notification);
