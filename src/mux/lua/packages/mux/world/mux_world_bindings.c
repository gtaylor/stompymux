/* mux_world_bindings.c - Lua bindings for mux.world. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/utf8.h"

/**
 * Privately emits a message to an object.
 *
 * @par Lua name `mux.world.pemit`
 * @par Lua signature `mux.world.pemit( object, message )`
 * @par Lua parameters - `object` (`number|Object`) The recipient.
 * - `message` (`string`) Valid UTF-8 text without embedded NUL bytes.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_CONNECTION_INVALID` for embedded NUL or invalid UTF-8;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid recipient.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_pemit(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  size_t length;
  const char *message = luaL_checklstring(state, 2, &length);

  if (lua_mux_package_is_checking(package))
    return lua_error_raise(state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
                           "mux.world.pemit is unavailable during @lua/check");
  if (strlen(message) != length)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CONNECTION_INVALID,
                         "message contains an embedded NUL byte");
  if (!utf8_validate(message, length))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CONNECTION_INVALID,
                         "message is not valid UTF-8");
  object = lua_mux_require_object(package, state, 1);
  notify_checked(&package->services->background_command->evaluation, object,
                 object, message, MSG_ME_ALL | MSG_F_DOWN);
  return 0;
}

void lua_mux_install_world_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_mux_install_object_bindings(state, package);
  lua_mux_install_state_bindings(state, package);
  lua_mux_install_attribute_bindings(state, package);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_pemit, 1);
  lua_setfield(state, -2, "pemit");
  lua_setfield(state, -2, "world");
}
