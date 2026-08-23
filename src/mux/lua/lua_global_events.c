/* Global Lua lifecycle event dispatch. */

#include <lua.h>
#include <stddef.h>

#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/support/alloc.h"

void lua_global_event_dispatch(LuaRuntime *runtime,
                               const LuaEventInvocation *invocation) {
  const char *event;

  if (!runtime || !invocation)
    return;
  event = lua_event_name(invocation->type);
  if (!event)
    return;

  for (size_t index = 0; index < runtime->global_module_count; index++) {
    lua_State *state = runtime->state;
    const char *path = lua_global_module_at(runtime, index);
    char error[LBUF_SIZE];
    int top = lua_gettop(state);
    int status;

    if (!lua_load_module(runtime, LUA_ROOT_GLOBAL_LOGIC, path, error,
                         sizeof(error))) {
      lua_log_load_error(runtime, NOTHING, path, error);
      lua_settop(state, top);
      continue;
    }
    lua_getfield(state, -1, "events");
    if (!lua_istable(state, -1)) {
      lua_settop(state, top);
      continue;
    }
    lua_getfield(state, -1, event);
    if (!lua_isfunction(state, -1)) {
      lua_settop(state, top);
      continue;
    }
    lua_push_context(runtime->services->database, invocation->descriptor, state,
                     NOTHING, invocation->enactor, invocation->cause, nullptr,
                     event, "global", invocation->arguments,
                     invocation->argument_count);
    if (invocation->type == LUA_EVENT_PLAYER_CONNECT) {
      lua_pushboolean(state, invocation->reconnect);
      lua_setfield(state, -2, "reconnect");
    } else if (invocation->type == LUA_EVENT_PLAYER_DISCONNECT &&
               invocation->reason) {
      lua_pushstring(state, invocation->reason);
      lua_setfield(state, -2, "reason");
    }
    {
      LuaModuleRoot previous_root = runtime->current_root;

      runtime->current_root = LUA_ROOT_GLOBAL_LOGIC;
      status = lua_callback_pcall_checked(runtime, 1, 0);
      runtime->current_root = previous_root;
    }
    if (status)
      lua_log_error_value(runtime, NOTHING, invocation->enactor, "EVENT", state,
                          -1);
    lua_settop(state, top);
  }
}
