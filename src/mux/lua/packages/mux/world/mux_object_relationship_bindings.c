/* mux_object_relationship_bindings.c - Lua bindings for object relationships.
 */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

/**
 * Returns this object's assigned zone.
 *
 * @par Lua name `object:zone`
 * @par Lua signature `object:zone( )`
 * @par Lua parameters - None.
 * @par Lua returns - `zone` (`Object|nil`): The assigned zone, or `nil` when
 * the object has no zone or its zone is being destroyed.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid receiver or stored zone.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_zone(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_mux_require_runtime(package, state, "object:zone");
  DbRef object = lua_mux_require_object(package, state, 1);
  DbRef zone = game_object_zone(package->services->database, object);

  if (zone == NOTHING) {
    lua_pushnil(state);
    return 1;
  }
  if (!is_good_obj(package->services->database, zone))
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_INVALID,
                           "object has an invalid zone");
  if (is_going(package->services->database, zone)) {
    lua_pushnil(state);
    return 1;
  }
  lua_mux_push_object(state, package, zone);
  return 1;
}

/**
 * Assigns this object's zone, or clears it with `nil`.
 *
 * @par Lua name `object:set_zone`
 * @par Lua signature `object:set_zone( zone )`
 * @par Lua parameters - `zone` (`number|Object|nil`) Live thing or room to
 * assign, or `nil` to clear the object's zone.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when the zone argument is omitted;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for invalid references or object kinds;
 * `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` for the receiver or zone being
 * destroyed.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_set_zone(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:set_zone");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (is_going(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is being destroyed");
  if (lua_gettop(state) < 2)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "zone is required; pass nil to clear it");
  if (lua_isnil(state, 2)) {
    game_object_set_zone(database, object, NOTHING);
    return 0;
  }

  DbRef zone = lua_mux_require_object(package, state, 2);
  if (is_going(database, zone))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "zone is being destroyed");
  if (!is_thing(database, zone) && !is_room(database, zone))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_INVALID,
                         "zone must be a thing or room");
  if (is_room(database, zone) && !is_room(database, object))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_INVALID,
                         "only rooms may use rooms as zones");

  game_object_set_zone(database, object, zone);
  return 0;
}

/**
 * Returns this object's assigned affiliation.
 *
 * @par Lua name `object:affiliation`
 * @par Lua signature `object:affiliation( )`
 * @par Lua parameters - None.
 * @par Lua returns - `affiliation` (`Object|nil`): The assigned affiliation,
 * or `nil` when the object has no affiliation or its affiliate is being
 * destroyed.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid receiver or stored
 * affiliation.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_affiliation(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_mux_require_runtime(package, state, "object:affiliation");
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
 * Assigns this object's affiliation, or clears it with `nil`.
 *
 * @par Lua name `object:set_affiliation`
 * @par Lua signature `object:set_affiliation( affiliation )`
 * @par Lua parameters - `affiliation` (`number|Object|nil`) Live object to
 * assign, or `nil` to clear the object's affiliation.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when the affiliation argument is omitted;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for invalid references;
 * `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` for the receiver or affiliation being
 * destroyed.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_set_affiliation(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:set_affiliation");
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

/**
 * Returns this object's direct Lua parent path.
 *
 * @par Lua name `object:lua_parent`
 * @par Lua signature `object:lua_parent( )`
 * @par Lua parameters - None.
 * @par Lua returns - `parent` (`string|nil`): The object's direct,
 * `object_logic`-relative Lua parent path, or `nil` when none is assigned.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid receiver.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_lua_parent(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_mux_require_runtime(package, state, "object:lua_parent");
  DbRef object = lua_mux_require_object(package, state, 1);
  const char *parent =
      game_object_lua_parent(package->services->database, object);

  if (*parent)
    lua_pushstring(state, parent);
  else
    lua_pushnil(state);
  return 1;
}

/**
 * Assigns this object's direct Lua parent path, or clears it with `nil`.
 *
 * @par Lua name `object:set_lua_parent`
 * @par Lua signature `object:set_lua_parent( parent )`
 * @par Lua parameters - `parent` (`string|nil`) Existing
 * `object_logic`-relative `.lua` path to assign, or `nil` to clear the
 * object's Lua parent.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` when the parent is omitted, is not a string
 * or nil, or contains an embedded NUL byte;
 * `LUA_ERROR_CODE_MODULE_INVALID` for an invalid or unavailable parent path;
 * `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid receiver;
 * `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` when the receiver is being destroyed or
 * the parent path cannot be stored.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_set_lua_parent(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:set_lua_parent");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (is_going(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is being destroyed");
  if (lua_gettop(state) < 2)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "parent is required; pass nil to clear it");
  if (lua_isnil(state, 2)) {
    if (!game_object_lua_parent_set(database, object, ""))
      return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                             "Lua parent could not be stored");
    return 0;
  }
  if (lua_type(state, 2) != LUA_TSTRING)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "parent must be a string or nil");

  size_t parent_length;
  const char *parent = lua_tolstring(state, 2, &parent_length);
  if (strlen(parent) != parent_length)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "parent contains an embedded NUL byte");
  char error[LBUF_SIZE];
  if (!lua_validate_path(package->context, parent, error, sizeof(error)))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_MODULE_INVALID, "%s", error);
  if (!game_object_lua_parent_set(database, object, parent))
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                           "Lua parent could not be stored");
  return 0;
}

void lua_mux_install_object_relationship_bindings(lua_State *state,
                                                  LuaMuxPackage *package) {
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_zone, 1);
  lua_setfield(state, -2, "zone");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_set_zone, 1);
  lua_setfield(state, -2, "set_zone");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_affiliation, 1);
  lua_setfield(state, -2, "affiliation");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_set_affiliation, 1);
  lua_setfield(state, -2, "set_affiliation");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_lua_parent, 1);
  lua_setfield(state, -2, "lua_parent");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_set_lua_parent, 1);
  lua_setfield(state, -2, "set_lua_parent");
}
