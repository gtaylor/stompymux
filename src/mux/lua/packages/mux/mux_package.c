/* mux_package.c - Built-in Lua mux package bindings. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/server/log.h"
#include "mux/world/database_check.h"

LuaMuxPackage *lua_mux_package_get(lua_State *state) {
  return lua_touserdata(state, lua_upvalueindex(1));
}

const char LUA_MUX_OBJECT_METATABLE[] = "btmux.object";
const char LUA_MUX_STATE_METATABLE[] = "btmux.object_state";
const char LUA_MUX_ATTRIBUTE_METATABLE[] = "btmux.object_attribute";
const char LUA_MUX_FLAGS_METATABLE[] = "btmux.object_flags";
const char LUA_MUX_POWERS_METATABLE[] = "btmux.object_powers";
const char LUA_MUX_FLAG_METATABLE[] = "btmux.flag";
const char LUA_MUX_POWER_METATABLE[] = "btmux.power";

bool lua_mux_package_transaction_begin(LuaMuxPackage *package) {
  return object_state_transaction_begin(&package->state_transaction,
                                        package->services->database);
}

void lua_mux_package_transaction_finish(LuaMuxPackage *package, bool commit) {
  object_state_transaction_finish(&package->state_transaction, commit);
}

void lua_mux_package_destroy(LuaMuxPackage *package) {
  object_state_transaction_destroy(&package->state_transaction);
}

bool lua_mux_package_is_checking(LuaMuxPackage *package) {
  return (package->is_checking && package->is_checking(package->context)) != 0;
}

void lua_mux_require_runtime(LuaMuxPackage *package, lua_State *state,
                             const char *function) {
  if (lua_mux_package_is_checking(package))
    lua_error_raise(state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
                    "mux.%s is unavailable during @lua/check", function);
}

/**
 * Checks the game database for inconsistencies and repairs any damage found.
 *
 * @par Lua name `mux.check_db`
 * @par Lua signature `mux.check_db( )`
 * @par Lua parameters - None.
 * @par Lua returns - None.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @par Lua notes Runs the same default consistency check as `@dbck`. Damage is
 * written to the server log, but no completion message is sent to a player.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_check_db(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_mux_require_runtime(package, state, "check_db");
  database_check(&(DatabaseCheckRequest){
      .evaluation = &package->services->background_command->evaluation,
      .player = NOTHING,
      .options = 0,
  });
  return 0;
}

/**
 * Appends a message to a named server log file.
 *
 * @par Lua name `mux.log`
 * @par Lua signature `mux.log( filename, message )`
 * @par Lua parameters - `filename` (`string`) The name of an existing readable
 * and writable file directly under game/logs/. Names may not contain /, ..,
 * embedded NUL bytes, or exceed 200 bytes.
 * - `message` (`string`) Text to append, followed by a newline. Embedded NUL
 * bytes are rejected.
 * @par Lua returns - `written` (`boolean`): Whether the named log accepted the
 * message.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`;
 * `LUA_ERROR_CODE_ARG_INVALID` for embedded NUL bytes.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_log(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  size_t filename_length;
  size_t message_length;
  const char *filename = luaL_checklstring(state, 1, &filename_length);
  const char *message = luaL_checklstring(state, 2, &message_length);

  lua_mux_require_runtime(package, state, "log");
  if (strlen(filename) != filename_length)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "filename contains an embedded NUL byte");
  if (strlen(message) != message_length)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "message contains an embedded NUL byte");
  lua_pushboolean(
      state,
      log_to_file(&(ArbitraryLogRequest){
          .evaluation = &package->services->background_command->evaluation,
          .actor = GOD,
          .filename = filename,
          .message = message}));
  return 1;
}

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package) {
  object_state_transaction_initialize(&package->state_transaction);
  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_check_db, 1);
  lua_setfield(state, -2, "check_db");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_log, 1);
  lua_setfield(state, -2, "log");
  lua_mux_install_world_bindings(state, package);
  lua_mux_install_session_bindings(state, package);
  lua_mux_install_text_bindings(state, package);
  lua_mux_install_telnet_bindings(state, package);
  lua_mux_install_error_bindings(state, package);
  lua_mux_install_config_bindings(state, package);
  lua_mux_install_comsys_bindings(state, package);
  lua_setglobal(state, "mux");
}
