#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* mux_session_bindings.c - Lua bindings for mux.session. */

#include <lauxlib.h>
#include <lua.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/network/descriptor.h"
#include "mux/objects/db.h"
#include "mux/server/server_config.h" // IWYU pragma: keep

/**
 * Attaches an interactive flow to a descriptor and shows its first prompt.
 *
 * @par LuaLS definition mux callable mux.session.flow_start
 * @code{.lua}
 * ---Attaches an interactive flow to a descriptor and displays its first prompt.
 * ---@param descriptor integer
 * ---@param module string Require-style module path.
 * ---@param first_step string Key in the module's `flows` table.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid), [`mux.error.codes.connection.unavailable`](lua://mux.error.codes.connection.unavailable), or [`mux.error.codes.module.invalid`](lua://mux.error.codes.module.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.connection.invalid
 * ---@see mux.error.codes.connection.unavailable
 * ---@see mux.error.codes.module.invalid
 * function mux_session.flow_start(descriptor, module, first_step) end
 * @endcode
 */
static int lua_mux_flow_start(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  int descriptor_id = (int)luaL_checkinteger(state, 1);
  const char *module = luaL_checkstring(state, 2);
  const char *first_step = luaL_checkstring(state, 3);

  if (lua_mux_package_is_checking(package))
    return lua_error_raise(
        state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
        "mux.session.flow_start is unavailable during @lua/check");
  if (!package->flow_start)
    return lua_error_raise(state, LUA_ERROR_CODE_CONNECTION_UNAVAILABLE,
                           "mux.session.flow_start is unavailable");
  return package->flow_start(package->context, state, descriptor_id, module,
                             first_step);
}

/**
 * Lists player connections visible to the normal who command.
 *
 * @par LuaLS definition mux callable mux.session.connected_players
 * @code{.lua}
 * ---Lists connected players visible to the ordinary WHO command.
 * ---@return Connection[] players
 * function mux_session.connected_players() end
 * @endcode
 */
static int lua_mux_connected_players(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor;
  DescriptorIterator iterator =
      descriptor_iterator_connected(package->services->descriptors);
  int index = 1;

  lua_newtable(state);
  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    lua_newtable(state);
    lua_mux_push_object(state, package, descriptor->player);
    lua_setfield(state, -2, "object");
    lua_pushstring(state, game_object_name(package->services->database,
                                           descriptor->player));
    lua_setfield(state, -2, "name");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->connected_at));
    lua_setfield(state, -2, "connected_for");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->last_time));
    lua_setfield(state, -2, "idle_for");
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

/**
 * Returns the non-privileged WHO summary.
 *
 * @par LuaLS definition mux callable mux.session.who_summary
 * @code{.lua}
 * ---Returns the non-privileged WHO summary.
 * ---@return WhoSummary summary
 * function mux_session.who_summary() end
 * @endcode
 */
static int lua_mux_who_summary(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_newtable(state);
  lua_pushinteger(state, 0);
  lua_setfield(state, -2, "hidden");
  lua_pushinteger(state, *package->services->record_players);
  lua_setfield(state, -2, "record");
  if (package->services->configuration->max_players == -1)
    lua_pushnil(state);
  else
    lua_pushinteger(state, package->services->configuration->max_players);
  lua_setfield(state, -2, "maximum");
  return 1;
}

/**
 * @par LuaLS definition mux type session.values
 * @code{.lua}
 * ---One player connection visible to the ordinary WHO command.
 * ---@class Connection
 * ---@field object Object Connected player.
 * ---@field name string Current object name.
 * ---@field connected_for integer Connected duration in seconds.
 * ---@field idle_for integer Idle duration in seconds.
 *
 * ---Non-privileged server population statistics.
 * ---@class WhoSummary
 * ---@field hidden integer Hidden-player count; currently always zero for this non-privileged view.
 * ---@field record integer Record simultaneous-player count.
 * ---@field maximum? integer Configured limit, or nil when unlimited.
 * @endcode
 *
 * @par LuaLS definition mux namespace mux.session
 * @code{.lua}
 * ---Live connection queries and interactive flows.
 * ---@class MuxSessionPackage
 * local mux_session = {}
 * @endcode
 */
void lua_mux_install_session_bindings(lua_State *state,
                                      LuaMuxPackage *package) {
  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_flow_start, 1);
  lua_setfield(state, -2, "flow_start");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_connected_players, 1);
  lua_setfield(state, -2, "connected_players");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_who_summary, 1);
  lua_setfield(state, -2, "who_summary");
  lua_setfield(state, -2, "session");
}
