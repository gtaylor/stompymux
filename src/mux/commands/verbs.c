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

void notify_action(EvaluationContext *evaluation, DbRef player, DbRef thing,
                   int what, const char *def, int owhat, const char *odef,
                   LuaEventType event, char *args[], int nargs) {
  char *d, *buff, *bp, *str;
  DbRef loc, aowner;
  long aflags;

  /*
   * message to player
   */

  if (what > 0) {
    d = attribute_parent_get(evaluation->world->database, thing, what, &aowner,
                             &aflags);
    if (*d) {
      buff = bp = alloc_lbuf("notify_action.1");
      str = d;
      exec(evaluation, buff, &bp, 0, thing, player,
           EV_EVAL | EV_FIGNORE | EV_TOP, &str, args, nargs);
      *bp = '\0';
      notify(evaluation, player, buff);
      free_lbuf(buff);
    } else if (def) {
      notify(evaluation, player, def);
    }
    free_lbuf(d);
  } else if ((what < 0) && def) {
    notify(evaluation, player, def);
  }
  /*
   * message to neighbors
   */

  if ((owhat > 0) && has_location(evaluation->world->database, player) &&
      is_good_obj(
          evaluation->world->database,
          loc = game_object_location(evaluation->world->database, player))) {
    d = attribute_parent_get(evaluation->world->database, thing, owhat, &aowner,
                             &aflags);
    if (*d) {
      buff = bp = alloc_lbuf("notify_action.2");
      str = d;
      exec(evaluation, buff, &bp, 0, thing, player,
           EV_EVAL | EV_FIGNORE | EV_TOP, &str, args, nargs);
      *bp = '\0';
      if (*buff)
        notify_except2(
            evaluation, loc, player, player, thing,
            tprintf("%s %s",
                    game_object_name(evaluation->world->database, player),
                    buff));
      free_lbuf(buff);
    } else if (odef) {
      notify_except2(
          evaluation, loc, player, player, thing,
          tprintf("%s %s",
                  game_object_name(evaluation->world->database, player), odef));
    }
    free_lbuf(d);
  } else if ((owhat < 0) && odef &&
             has_location(evaluation->world->database, player) &&
             is_good_obj(evaluation->world->database,
                         loc = game_object_location(evaluation->world->database,
                                                    player))) {
    notify_except2(
        evaluation, loc, player, player, thing,
        tprintf("%s %s", game_object_name(evaluation->world->database, player),
                odef));
  }
  if (event != LUA_EVENT_NONE) {
    LuaEventInvocation invocation = {
        .type = event,
        .object = thing,
        .enactor = player,
        .cause = player,
        .arguments = args,
        .argument_count = nargs,
    };

    lua_event_dispatch(evaluation->runtime->lua_owner->runtime, &invocation);
  }
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
