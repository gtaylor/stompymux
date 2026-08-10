/* access.c - Object visibility, lock, and hearing permission checks. */

#include "mux/world/access.h"

#include <string.h>

#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"

bool lock_evaluate(EvaluationContext *context,
                   const LuaLockInvocation *invocation, LuaLockResult *result) {
  memset(result, 0, sizeof(*result));
  lua_lock_evaluate(context->runtime->lua_owner->runtime, invocation, result);
  return result->passes;
}

bool lock_test(EvaluationContext *context, DbRef enactor, DbRef cause,
               DbRef subject, DbRef object, LuaLockType type,
               LuaLockOperation operation, bool silent,
               LuaLockInvocation *invocation, LuaLockResult *result) {
  *invocation = (LuaLockInvocation){
      .type = type,
      .operation = operation,
      .descriptor = context->command ? context->command->descriptor : nullptr,
      .object = object,
      .enactor = enactor,
      .cause = cause,
      .subject = subject,
      .silent = silent,
  };
  return lock_evaluate(context, invocation, result);
}

bool can_see(const ObjectVisibilityRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->viewer;
  DbRef thing = request->object;
  /*
   * Don't show if all the following apply: * Sleeping players should *
   *
   * *  * * not be seen. * The thing is a disconnected player. * The
   * viewer cannot see the disconnected player.
   */

  if (is_player(evaluation->world->database, thing) &&
      !is_connected(evaluation->world->database, thing)) {
    return 0;
  }
  /*
   * You don't see yourself or exits
   */

  if ((player == thing) || is_exit(evaluation->world->database, thing)) {
    return 0;
  }
  /* In visible locations, OBJECT_FLAG_DARK objects remain hidden. In
   * OBJECT_FLAG_DARK locations, only LIGHT objects that are not themselves
   * OBJECT_FLAG_DARK are visible. */

  if (request->location_visible)
    return !is_dark(evaluation->world->database, thing);
  return is_light(evaluation->world->database, thing) &&
         !is_dark(evaluation->world->database, thing);
}
