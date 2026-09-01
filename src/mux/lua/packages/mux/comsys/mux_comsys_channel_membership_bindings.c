/* mux_comsys_channel_membership_bindings.c - Lua channel membership methods. */

#include <lua.h>
#include <stddef.h>
#include <string.h>

#include "mux/communication/comsys.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/comsys/mux_comsys_bindings_internal.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

/**
 * Adds a player to a channel with a player-local command alias.
 *
 * @par Lua name `Channel:add_player`
 * @par Lua signature `channel:add_player( player, alias, quiet )`
 * @par Lua parameters - `player` (`number|Object`) Live player object.
 * - `alias` (`string`) One to five printable ASCII characters without spaces.
 * - `quiet` (`boolean`) Suppresses the channel-wide join announcement.
 * @par Lua returns - None.
 * @par Lua errors - Argument, object, or channel validation errors; unavailable
 * while checking.
 * @par Lua availability Available only at runtime.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
int lua_mux_channel_add_player(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);
  DbRef player = lua_mux_require_object(handle->package, state, 2);
  size_t alias_length;
  const char *alias;

  if (typeof_obj(handle->package->services->database, player) !=
      OBJECT_TYPE_PLAYER)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object must be a player");
  if (is_going(handle->package->services->database, player))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "player is being destroyed");
  if (lua_type(state, 3) != LUA_TSTRING)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "alias must be a string");
  alias = lua_tolstring(state, 3, &alias_length);
  if (strlen(alias) != alias_length)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "alias contains an embedded NUL byte");
  if (!lua_isboolean(state, 4))
    return lua_error_arg(state, 4, LUA_ERROR_CODE_ARG_INVALID,
                         "quiet must be a boolean");

  ChannelAddPlayerResult result = comsys_channel_add_player(
      &handle->package->services->background_command->evaluation, player,
      handle->identity, alias, lua_toboolean(state, 4) != 0);
  switch (result) {
  case CHANNEL_ADD_PLAYER_OK:
    return 0;
  case CHANNEL_ADD_PLAYER_ALIAS_REQUIRED:
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "alias is required");
  case CHANNEL_ADD_PLAYER_ALIAS_INVALID:
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "alias must be 1-5 printable ASCII characters "
                         "without spaces");
  case CHANNEL_ADD_PLAYER_ALIAS_IN_USE:
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "alias is already in use");
  case CHANNEL_ADD_PLAYER_CAPACITY_FAILURE:
    return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                           "unable to add channel alias");
  }
  return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                         "unknown add-player result");
}
