/** @file
 * Object visibility, lock, and hearing permission interfaces.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

/** Executes lock evaluate. @param[in,out] context Operation context. @param[in]
 * invocation Command invocation. @param[out] result Result. */

bool lock_evaluate(EvaluationContext *context,
                   const LuaLockInvocation *invocation, LuaLockResult *result);
/** Executes lock test. @param[in,out] context Operation context. @param[in]
 * enactor Object that initiated the operation. @param[in] cause Object that
 * caused the operation. @param[in] subject Subject. @param[in] object Game
 * object. @param[in] type Type. @param[in] operation Operation. @param[in]
 * silent Silent. @param[in,out] invocation Command invocation. @param[out]
 * result Result. */

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

/** Reports whether can see. @param[in] request Request. */

bool can_see(const ObjectVisibilityRequest *request);
