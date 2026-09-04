/* mux_telnet_bindings.c - Lua bindings for mux.telnet. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/network/descriptor.h"
#include "mux/network/telnet_environment.h"

static Descriptor *lua_mux_require_descriptor(LuaMuxPackage *package,
                                              lua_State *state, int argument) {
  int descriptor_id = (int)luaL_checkinteger(state, argument);
  Descriptor *descriptor;

  lua_mux_require_runtime(package, state, "telnet.environment");
  descriptor =
      descriptor_find_by_fd(package->services->descriptors, descriptor_id);
  if (descriptor == nullptr)
    lua_error_arg(state, argument, LUA_ERROR_CODE_CONNECTION_INVALID,
                  "no such descriptor");
  return descriptor;
}

static TelnetEnvironmentKind lua_mux_telnet_environment_kind(lua_State *state,
                                                             int argument) {
  const char *kind = luaL_checkstring(state, argument);

  if (!strcmp(kind, "var"))
    return TELNET_ENVIRONMENT_VAR;
  if (!strcmp(kind, "uservar"))
    return TELNET_ENVIRONMENT_USERVAR;
  lua_error_arg(state, argument, LUA_ERROR_CODE_CONNECTION_INVALID,
                "kind must be 'var' or 'uservar'");
  return TELNET_ENVIRONMENT_VAR;
}

/**
 * Tests whether an RFC 1572 NEW-ENVIRON variable is defined on a live
 * connection.
 *
 * @par LuaLS definition mux callable mux.telnet.environment_has
 * @code{.lua}
 * ---Tests whether a binary-safe RFC 1572 NEW-ENVIRON variable is defined.
 * ---@param descriptor integer Live descriptor ID, normally `ctx.descriptor`.
 * ---@param kind TelnetEnvironmentKind NEW-ENVIRON variable namespace.
 * ---@param name string Binary-safe variable name.
 * ---@return boolean defined Whether the variable is present, including with an empty value.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.connection.invalid
 * function mux_telnet.environment_has(descriptor, kind, name) end
 * @endcode
 */
static int lua_mux_telnet_environment_has(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor = lua_mux_require_descriptor(package, state, 1);
  TelnetEnvironmentKind kind = lua_mux_telnet_environment_kind(state, 2);
  size_t name_size;
  const char *name = luaL_checklstring(state, 3, &name_size);
  bool has_environment =
      descriptor_telnet_environment_has(descriptor, kind, name, name_size);

  lua_pushboolean(state, (int)has_environment);
  return 1;
}

/**
 * Gets an RFC 1572 NEW-ENVIRON variable from a live connection.
 *
 * @par LuaLS definition mux callable mux.telnet.environment_get
 * @code{.lua}
 * ---Gets a binary-safe RFC 1572 NEW-ENVIRON value.
 * ---@param descriptor integer Live descriptor ID, normally `ctx.descriptor`.
 * ---@param kind TelnetEnvironmentKind NEW-ENVIRON variable namespace.
 * ---@param name string Binary-safe variable name.
 * ---@return string? value Binary-safe value, or nil when the variable is absent.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.connection.invalid`](lua://mux.error.codes.connection.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.connection.invalid
 * function mux_telnet.environment_get(descriptor, kind, name) end
 * @endcode
 */
static int lua_mux_telnet_environment_get(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor = lua_mux_require_descriptor(package, state, 1);
  TelnetEnvironmentKind kind = lua_mux_telnet_environment_kind(state, 2);
  size_t name_size;
  const char *name = luaL_checklstring(state, 3, &name_size);
  const void *value;
  size_t value_size;

  if (descriptor_telnet_environment_get(descriptor, kind, name, name_size,
                                        &value, &value_size))
    lua_pushlstring(state, value, value_size);
  else
    lua_pushnil(state);
  return 1;
}

/**
 * @par LuaLS definition mux alias telnet.environment_kind
 * @code{.lua}
 * ---@alias TelnetEnvironmentKind "var"|"uservar" RFC 1572 NEW-ENVIRON variable namespace.
 * @endcode
 *
 * @par LuaLS definition mux namespace mux.telnet
 * @code{.lua}
 * ---Telnet protocol state and capabilities for live connections.
 * ---@class MuxTelnetPackage
 * local mux_telnet = {}
 * @endcode
 */
void lua_mux_install_telnet_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_telnet_environment_has, 1);
  lua_setfield(state, -2, "environment_has");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_telnet_environment_get, 1);
  lua_setfield(state, -2, "environment_get");
  lua_setfield(state, -2, "telnet");
}
