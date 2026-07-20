/* access.h - Object visibility, lock, and hearing permission interfaces. */

#pragma once

#include "mux/database/db.h"
#include "mux/lua/lua_runtime.h"

typedef struct ServerConfiguration ServerConfiguration;

typedef struct EvaluationContext EvaluationContext;

bool lock_evaluate(EvaluationContext *context,
                   const LuaLockInvocation *invocation, LuaLockResult *result);
bool lock_test(EvaluationContext *context, DbRef enactor, DbRef cause,
               DbRef subject, DbRef object, LuaLockType type,
               LuaLockOperation operation, bool silent,
               LuaLockInvocation *invocation, LuaLockResult *result);
int can_see(EvaluationContext *evaluation,
            const ServerConfiguration *configuration, DbRef player, DbRef thing,
            int can_see_location);
void handle_ears(EvaluationContext *evaluation, DbRef thing, int could_hear,
                 int can_hear);
