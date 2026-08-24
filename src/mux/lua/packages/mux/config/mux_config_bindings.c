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
 * @par Lua name `mux.config.get`
 * @par Lua signature `mux.config.get( name )`
 * @par Lua parameters - `name` (`string`) Exact, case-sensitive configuration
 * directive name. Embedded NUL bytes are rejected.
 * @par Lua returns - `value` (`string|number|boolean`): The directive's current
 * live value, represented using its native Lua scalar type.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for an embedded NUL byte;
 * `LUA_ERROR_CODE_CONFIG_NOT_FOUND` for an unknown directive;
 * `LUA_ERROR_CODE_CONFIG_UNSUPPORTED` for a known non-scalar directive.
 * @par Lua availability Available both at runtime and during `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and result is
 * pushed.
 * @return The number of Lua values pushed onto the stack.
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

void lua_mux_install_config_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_config_get, 1);
  lua_setfield(state, -2, "get");
  lua_setfield(state, -2, "config");
}
