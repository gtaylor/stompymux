/* access.h - Object visibility, lock, and hearing permission interfaces. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

bool lock_evaluate(EvaluationContext *context,
                   const LuaLockInvocation *invocation, LuaLockResult *result);
bool lock_test(EvaluationContext *context, DbRef enactor, DbRef cause,
               DbRef subject, DbRef object, LuaLockType type,
               LuaLockOperation operation, bool silent,
               LuaLockInvocation *invocation, LuaLockResult *result);
typedef struct ObjectVisibilityRequest {
  EvaluationContext *evaluation;
  DbRef viewer;
  DbRef object;
  bool location_visible;
} ObjectVisibilityRequest;

bool can_see(const ObjectVisibilityRequest *request);
