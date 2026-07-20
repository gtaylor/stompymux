/* verbs.h - Action messaging and native Lua event dispatch interface. */

#pragma once

#include "mux/database/db.h"
#include "mux/lua/lua_runtime.h"

typedef struct EvaluationContext EvaluationContext;

void notify_action(EvaluationContext *evaluation, DbRef player, DbRef thing,
                   int player_attribute, const char *player_default,
                   int others_attribute, const char *others_default,
                   LuaEventType event, char *arguments[], int argument_count);
void notify_lock_failure(EvaluationContext *evaluation,
                         const LuaLockInvocation *invocation,
                         const LuaLockResult *result,
                         const char *enactor_default, const char *other_default,
                         LuaEventType event);
