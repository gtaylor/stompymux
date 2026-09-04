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
 * @par LuaLS definition mux callable mux.check_db
 * @code{.lua}
 * ---Checks the database for inconsistencies and repairs damage found by the
 * ---default native `@dbck` pass. Findings are written to the server log.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking).
 * ---@see mux.error.codes.unavailable.checking
 * function mux.check_db() end
 * @endcode
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
 * @par LuaLS definition mux callable mux.log
 * @code{.lua}
 * ---Appends a newline-terminated message to a permitted file under `game/logs`.
 * ---@param filename string
 * ---@param message string
 * ---@return boolean written
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * function mux.log(filename, message) end
 * @endcode
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

/**
 * @par LuaLS definition mux alias dbref
 * @code{.lua}
 * ---@alias DbRef integer Database object reference.
 * @endcode
 *
 * @par LuaLS definition mux namespace mux
 * @code{.lua}
 * ---The native MUX host API.
 * ---@class MuxPackage
 * ---@field comsys MuxComsysPackage Trusted live communication-channel administration.
 * ---@field config MuxConfigPackage Read-only scalar server configuration.
 * ---@field error MuxErrorPackage Structured errors and checked code nodes.
 * ---@field session MuxSessionPackage Live connections and interactive flows.
 * ---@field telnet MuxTelnetPackage Telnet protocol state and capabilities.
 * ---@field text MuxTextPackage Styled-text utilities.
 * ---@field world MuxWorldPackage World database object access.
 * mux = {}
 * @endcode
 *
 * @par LuaLS definition mux binding mux.packages
 * @code{.lua}
 * mux.config = mux_config
 * mux.comsys = mux_comsys
 * mux.error = mux_error
 * mux.session = mux_session
 * mux.telnet = mux_telnet
 * mux.text = mux_text
 * mux.world = mux_world
 * @endcode
 */
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
