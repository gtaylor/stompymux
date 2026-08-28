/* mux_affiliation_bindings.c - Lua bindings for object affiliations. */

#include <lauxlib.h>
#include <lua.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"

/**
 * Returns an object's assigned affiliation.
 *
 * @par Lua name `mux.world.affiliation`
 * @par Lua signature `mux.world.affiliation( object )`
 * @par Lua parameters - `object` (`number|Object`) Live object to inspect.
 * @par Lua returns - `affiliation` (`Object|nil`): The assigned affiliation,
 * or `nil` when the object has no affiliation or its affiliate is being
 * destroyed.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid reference.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_affiliation(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_mux_require_runtime(package, state, "world.affiliation");
  DbRef object = lua_mux_require_object(package, state, 1);
  DbRef affiliation =
      game_object_affiliation(package->services->database, object);

  if (affiliation == NOTHING) {
    lua_pushnil(state);
    return 1;
  }
  if (!is_good_obj(package->services->database, affiliation))
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_INVALID,
                           "object has an invalid affiliation");
  if (is_going(package->services->database, affiliation)) {
    lua_pushnil(state);
    return 1;
  }
  lua_mux_push_object(state, package, affiliation);
  return 1;
}

/**
 * Assigns an object's affiliation, or clears it with `nil`.
 *
 * @par Lua name `mux.world.set_affiliation`
 * @par Lua signature `mux.world.set_affiliation( object, affiliation )`
 * @par Lua parameters - `object` (`number|Object`) Live object to update.
 * - `affiliation` (`number|Object|nil`) Live object to assign, or `nil` to
 * clear the object's affiliation.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when the affiliation argument is omitted;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for invalid references;
 * `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` for an object or affiliation being
 * destroyed.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_set_affiliation(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "world.set_affiliation");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (is_going(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is being destroyed");
  if (lua_gettop(state) < 2)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "affiliation is required; pass nil to clear it");
  if (lua_isnil(state, 2)) {
    game_object_set_affiliation(database, object, NOTHING);
    return 0;
  }

  DbRef affiliation = lua_mux_require_object(package, state, 2);
  if (is_going(database, affiliation))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "affiliation is being destroyed");

  game_object_set_affiliation(database, object, affiliation);
  return 0;
}

void lua_mux_install_affiliation_bindings(lua_State *state,
                                          LuaMuxPackage *package) {
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_affiliation, 1);
  lua_setfield(state, -2, "affiliation");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_set_affiliation, 1);
  lua_setfield(state, -2, "set_affiliation");
}
