/* mux_config_bindings.c - Read-only Lua configuration queries. */

#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/server/configuration_registry.h"

/**
 * Returns the live value of a scalar server configuration directive.
 *
 * @par LuaLS definition mux callable mux.config.get
 * @code{.lua}
 * ---Returns the live scalar value of an exact, case-sensitive configuration directive.
 * ---@param name string Configuration directive name; embedded NUL bytes are rejected.
 * ---@return ConfigValue value Current value represented by its native Lua scalar type.
 * ---
 * ---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.config.not_found`](lua://mux.error.codes.config.not_found), [`mux.error.codes.config.unsupported`](lua://mux.error.codes.config.unsupported), or [`mux.error.codes.internal`](lua://mux.error.codes.internal).
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.config.not_found
 * ---@see mux.error.codes.config.unsupported
 * ---@see mux.error.codes.internal
 * function mux_config.get(name) end
 * @endcode
 */
static int lua_mux_config_get(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  size_t name_length;
  const char *name;
  ConfigurationValue value;

  if (lua_type(state, 1) != LUA_TSTRING)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "configuration name must be a string");
  name = lua_tolstring(state, 1, &name_length);
  if (strlen(name) != name_length)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "configuration name contains an embedded NUL byte");

  const ConfigurationQueryStatus STATUS = configuration_registry_query(
      package->services->configuration_registry,
      package->services->configuration, name, &value);
  if (STATUS == CONFIGURATION_QUERY_NOT_FOUND)
    return lua_error_raise(state, LUA_ERROR_CODE_CONFIG_NOT_FOUND,
                           "unknown configuration directive '%s'", name);
  if (STATUS == CONFIGURATION_QUERY_UNSUPPORTED)
    return lua_error_raise(state, LUA_ERROR_CODE_CONFIG_UNSUPPORTED,
                           "configuration directive '%s' is not a readable "
                           "scalar",
                           name);

  switch (value.kind) {
  case CONFIGURATION_VALUE_INTEGER:
    lua_pushinteger(state, value.as.integer);
    break;
  case CONFIGURATION_VALUE_NUMBER:
    lua_pushnumber(state, value.as.number);
    break;
  case CONFIGURATION_VALUE_BOOLEAN:
    lua_pushboolean(state, value.as.boolean ? 1 : 0);
    break;
  case CONFIGURATION_VALUE_INTEGER_BOOLEAN:
    return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                           "configuration query returned an invalid type");
  case CONFIGURATION_VALUE_STRING:
    lua_pushstring(state, value.as.string);
    break;
  case CONFIGURATION_VALUE_UNSUPPORTED:
  case CONFIGURATION_VALUE_LUA_ERROR_REPORTING:
    return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                           "configuration query returned an invalid type");
  }
  return 1;
}

/**
 * @par LuaLS definition mux alias config.value
 * @code{.lua}
 * ---@alias ConfigValue string|number|boolean Scalar value returned by the live configuration registry.
 * @endcode
 *
 * @par LuaLS definition mux namespace mux.config
 * @code{.lua}
 * ---Read-only access to live scalar server configuration.
 * ---@class MuxConfigPackage
 * local mux_config = {}
 * @endcode
 */
void lua_mux_install_config_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_config_get, 1);
  lua_setfield(state, -2, "get");
  lua_setfield(state, -2, "config");
}
