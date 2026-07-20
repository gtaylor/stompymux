/* verbs.h - Action messaging and native Lua event dispatch interface. */

#pragma once

#include "mux/database/db.h"
#include "mux/lua/lua_runtime.h"

typedef struct EvaluationContext EvaluationContext;

typedef struct ActionMessageInvocation {
  LuaMessageInvocation message;
  int content_attribute;
  const char *enactor_default;
  const char *other_default;
  LuaEventType event;
  char **arguments;
  int argument_count;
} ActionMessageInvocation;

void notify_action(EvaluationContext *evaluation,
                   const ActionMessageInvocation *invocation);
void notify_event(EvaluationContext *evaluation, Descriptor *descriptor,
                  DbRef enactor, DbRef cause, DbRef object, LuaEventType event,
                  char **arguments, int argument_count);
void notify_lock_failure(EvaluationContext *evaluation,
                         const LuaLockInvocation *invocation,
                         const LuaLockResult *result,
                         const char *enactor_default, const char *other_default,
                         LuaEventType event);
