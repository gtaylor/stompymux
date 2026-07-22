/* access.c - Object visibility, lock, and hearing permission checks. */

#include "mux/world/access.h"

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_runtime.h"
#include "mux/database/powers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/formatting.h"
#include "mux/world/world_context.h"

#include <string.h>

bool lock_evaluate(EvaluationContext *context,
                   const LuaLockInvocation *invocation, LuaLockResult *result) {
  memset(result, 0, sizeof(*result));
  if (is_pass_locks(context->world->database, invocation->subject)) {
    result->defined = lua_lock_defined(context->runtime->lua_owner->runtime,
                                       invocation->object, invocation->type);
    result->passes = true;
    return true;
  }
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

int can_see(EvaluationContext *evaluation,
            const ServerConfiguration *configuration, DbRef player, DbRef thing,
            int can_see_loc) {
  /*
   * Don't show if all the following apply: * Sleeping players should *
   *
   * *  * * not be seen. * The thing is a disconnected player. * The
   * viewer cannot see the disconnected player.
   */

  if (configuration->dark_sleepers &&
      is_player(evaluation->world->database, thing) &&
      !is_connected(evaluation->world->database, thing)) {
    return 0;
  }
  /*
   * You don't see yourself or exits
   */

  if ((player == thing) || is_exit(evaluation->world->database, thing)) {
    return 0;
  }
  /* In visible locations, DARK objects remain hidden. In DARK locations,
   * only LIGHT objects that are not themselves DARK are visible. */

  if (can_see_loc)
    return !is_dark(evaluation->world->database, thing);
  return is_light(evaluation->world->database, thing) &&
         !is_dark(evaluation->world->database, thing);
}
void handle_ears(EvaluationContext *evaluation, DbRef thing, int could_hear,
                 int can_hear) {
  char *buff, *bp;

  if (!could_hear && can_hear) {
    buff = alloc_lbuf("handle_ears.grow");
    StringCopy(buff, game_object_name(evaluation->world->database, thing));
    if (is_exit(evaluation->world->database, thing)) {
      for (bp = buff; *bp && (*bp != ';'); bp++)
        ;
      *bp = '\0';
    }
    notify_checked(evaluation, thing, thing,
                   tprintf("%s grows ears and can now hear.", buff),
                   (MSG_ME | MSG_NBR | MSG_LOC | MSG_INV));
    free_lbuf(buff);
  } else if (could_hear && !can_hear) {
    buff = alloc_lbuf("handle_ears.lose");
    StringCopy(buff, game_object_name(evaluation->world->database, thing));
    if (is_exit(evaluation->world->database, thing)) {
      for (bp = buff; *bp && (*bp != ';'); bp++)
        ;
      *bp = '\0';
    }
    notify_checked(evaluation, thing, thing,
                   tprintf("%s loses its ears and becomes deaf.", buff),
                   (MSG_ME | MSG_NBR | MSG_LOC | MSG_INV));
    free_lbuf(buff);
  }
}
