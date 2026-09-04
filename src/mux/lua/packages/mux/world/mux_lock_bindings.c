/* mux_lock_bindings.c - Lua bindings for testing object locks. */

#include <lauxlib.h>
#include <lua.h>

#include "mux/commands/command_context.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_lock_catalog.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/server/platform.h"

static const char LUA_MUX_LOCK_METATABLE[] = "btmux.lock";
static const char LUA_MUX_LOCK_NAMESPACE_METATABLE[] = "btmux.lock_namespace";

typedef struct LuaMuxLock {
  LuaMuxPackage *package;
  const LuaLockDefinition *definition;
} LuaMuxLock;

typedef struct LuaMuxLockNamespace {
  LuaMuxPackage *package;
} LuaMuxLockNamespace;

/**
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the Lock class declaration.
 */
static int lua_mux_lock_tostring(lua_State *state) {
  LuaMuxLock *lock = luaL_checkudata(state, 1, LUA_MUX_LOCK_METATABLE);

  lua_pushstring(state, lock->definition->constant);
  return 1;
}

/**
 * @par LuaLS ignore mux __eq -- LuaCATS has no equality-operator declaration; Lock equality semantics are documented on the class.
 */
static int lua_mux_lock_equal(lua_State *state) {
  LuaMuxLock *left = luaL_checkudata(state, 1, LUA_MUX_LOCK_METATABLE);
  LuaMuxLock *right = luaL_checkudata(state, 2, LUA_MUX_LOCK_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->definition == right->definition);
  return 1;
}

/**
 * @par LuaLS ignore mux __newindex -- Immutability is represented by the Lock class and lock namespace table declarations.
 */
static int lua_mux_lock_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "mux.world.locks constants are immutable");
}

/**
 * @par LuaLS ignore mux __index -- Dynamic lookup is represented by the lock namespace table declaration.
 */
static int lua_mux_lock_namespace_index(lua_State *state) {
  LuaMuxLockNamespace *name_space =
      luaL_checkudata(state, 1, LUA_MUX_LOCK_NAMESPACE_METATABLE);
  const char *name = lua_tostring(state, 2);

  if (!name)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "lock name must be a string");
  const LuaLockDefinition *definition = lua_lock_definition_find_constant(name);
  if (!definition)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "unknown lock '%s'", name);
  LuaMuxLock *lock = lua_newuserdata(state, sizeof(*lock));

  *lock =
      (LuaMuxLock){.package = name_space->package, .definition = definition};
  luaL_getmetatable(state, LUA_MUX_LOCK_METATABLE);
  lua_setmetatable(state, -2);
  return 1;
}

/**
 * Tests a native lock invocation without emitting lock messages.
 *
 * @par LuaLS definition mux callable mux.world.lock_passes
 * @code{.lua}
 * ---Tests a native object lock without emitting lock messages or performing the
 * ---associated action. The lock runs with a silent callback context.
 * ---@param options LockPassesOptions Lock invocation fields; unknown fields are rejected.
 * ---@return boolean passes Whether the selected lock passes.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function mux_world.lock_passes(options) end
 * @endcode
 */
static int lua_mux_lock_passes(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  static const char *const FIELDS[] = {"object", "enactor", "lock", "cause",
                                       "subject"};
  bool present;

  lua_mux_require_runtime(package, state, "world.lock_passes");
  lua_mux_check_options(state, 1, FIELDS, sizeof(FIELDS) / sizeof(*FIELDS));
  DbRef object =
      lua_mux_option_object(package, state, 1, "object", true, &present);
  DbRef enactor =
      lua_mux_option_object(package, state, 1, "enactor", true, &present);
  DbRef cause =
      lua_mux_option_object(package, state, 1, "cause", false, &present);
  if (!present)
    cause = enactor;
  DbRef subject =
      lua_mux_option_object(package, state, 1, "subject", false, &present);
  if (!present)
    subject = enactor;

  lua_getfield(state, 1, "lock");
  LuaMuxLock *lock = luaL_testudata(state, -1, LUA_MUX_LOCK_METATABLE);
  if (!lock || lock->package != package)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "options.lock must be a mux.world.locks "
                         "constant from this runtime");
  const LuaLockDefinition *definition = lock->definition;

  lua_pop(state, 1);
  if (!package->lock_passes)
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                           "mux.world.lock_passes is unavailable");
  bool passes = package->lock_passes(
      package->context,
      &(LuaLockInvocation){
          .type = definition->type,
          .descriptor = package->services->background_command->descriptor,
          .object = object,
          .enactor = enactor,
          .cause = cause,
          .subject = subject,
          .silent = true,
      });

  lua_pushboolean(state, (int)passes);
  return 1;
}

/**
 * Installs typed constants for native locks.
 *
 * @par LuaLS definition mux type lock
 * @code{.lua}
 * ---A typed native lock obtained from [`mux.world.locks`](lua://mux.world.locks).
 * ---Its string form is its uppercase name, and equality compares lock identity
 * ---within the current runtime.
 * ---@class Lock
 * @endcode
 *
 * @param[in,out] state Lua state whose top value is the `mux.world` table.
 * @param[in,out] package Package owning the constants.
 */
void lua_mux_install_lock_bindings(lua_State *state, LuaMuxPackage *package) {
  luaL_newmetatable(state, LUA_MUX_LOCK_METATABLE);
  lua_pushcfunction(state, lua_mux_lock_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_lock_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_mux_lock_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_LOCK_NAMESPACE_METATABLE);
  lua_pushcfunction(state, lua_mux_lock_namespace_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_lock_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  LuaMuxLockNamespace *name_space = lua_newuserdata(state, sizeof(*name_space));
  *name_space = (LuaMuxLockNamespace){.package = package};
  luaL_getmetatable(state, LUA_MUX_LOCK_NAMESPACE_METATABLE);
  lua_setmetatable(state, -2);
  lua_setfield(state, -2, "locks");

  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_lock_passes, 1);
  lua_setfield(state, -2, "lock_passes");
}
