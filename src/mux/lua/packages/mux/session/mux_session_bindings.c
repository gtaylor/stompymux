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
 * @par Lua name `mux.session.flow_start`
 * @par Lua signature `mux.session.flow_start( descriptor, module, first_step )`
 * @par Lua parameters - `descriptor` (`number`) A live descriptor ID, normally
 * ctx.descriptor.
 * - `module` (`string`) The flow module path, resolved like require.
 * - `first_step` (`string`) A key in the module's flows table.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`;
 * `LUA_ERROR_CODE_CONNECTION_UNAVAILABLE` when flow support is absent.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
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
 * @par Lua name `mux.session.connected_players`
 * @par Lua signature `mux.session.connected_players( )`
 * @par Lua parameters - None.
 * @par Lua returns - `players` (`Connection[]`): Connection records with
 * `object` (`Object`), `name` (`string`), `connected_for` (`integer` seconds),
 * and `idle_for` (`integer` seconds).
 * @par Lua errors - No stable native error is raised after Lua argument
 * validation.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
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
 * @par Lua name `mux.session.who_summary`
 * @par Lua signature `mux.session.who_summary( )`
 * @par Lua parameters - None.
 * @par Lua returns - `summary` (`WhoSummary`): A table with `hidden`
 * (`integer`), `record` (`integer`), and `maximum` (`integer|nil`) fields.
 * @par Lua errors - No stable native error is raised after Lua argument
 * validation.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
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
