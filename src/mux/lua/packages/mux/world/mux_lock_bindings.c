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

static int lua_mux_lock_tostring(lua_State *state) {
  LuaMuxLock *lock = luaL_checkudata(state, 1, LUA_MUX_LOCK_METATABLE);

  lua_pushstring(state, lock->definition->constant);
  return 1;
}

static int lua_mux_lock_equal(lua_State *state) {
  LuaMuxLock *left = luaL_checkudata(state, 1, LUA_MUX_LOCK_METATABLE);
  LuaMuxLock *right = luaL_checkudata(state, 2, LUA_MUX_LOCK_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->definition == right->definition);
  return 1;
}

static int lua_mux_lock_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "mux.world.locks constants are immutable");
}

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
 * @par Lua name `mux.world.lock_passes`
 * @par Lua signature `mux.world.lock_passes( options )`
 * @par Lua parameters - `options` (`LockPassesOptions`) Required `object`,
 * `enactor`, and typed `lock` fields, with optional `cause` and `subject`
 * object references that each default to `enactor`.
 * @par Lua returns - `passes` (`boolean`): Whether the selected lock passes.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` for invalid fields or lock constants.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for invalid object references.
 * - `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` when the lock service is unavailable.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
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
 * @par Lua name `mux.world.locks`
 * @par Lua constants Flat immutable `Lock` constants such as `TRAVERSE` and
 * `CHANNEL_RECEIVE`.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for unknown or non-string
 * lookups and attempted mutation.
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
