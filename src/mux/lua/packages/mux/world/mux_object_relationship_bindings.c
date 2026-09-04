/* mux_object_relationship_bindings.c - Lua bindings for object relationships.
 */

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
#include "mux/support/alloc.h"

static int lua_mux_push_relationship(lua_State *state, LuaMuxPackage *package,
                                     DbRef relationship,
                                     const char *relationship_name) {
  GameDatabase *database = package->services->database;

  if (relationship == NOTHING || (is_good_obj(database, relationship) &&
                                  is_going(database, relationship))) {
    lua_pushnil(state);
    return 1;
  }
  if (!is_good_obj(database, relationship))
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_INVALID,
                           "object has an invalid %s", relationship_name);
  lua_mux_push_object(state, package, relationship);
  return 1;
}

/**
 * Returns this exit's destination.
 *
 * @par LuaLS definition mux callable Object:destination
 * @code{.lua}
 * ---Returns this exit's destination, or nil when it is unlinked or the
 * ---destination is being destroyed.
 * ---@return Object? destination
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---during `@lua/check`, or
 * ---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
 * ---the receiver is not an exit or its stored destination is invalid.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * function Object:destination() end
 * @endcode
 */
static int lua_mux_object_destination(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:destination");
  DbRef exit = lua_mux_require_object(package, state, 1);
  if (!is_exit(database, exit))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not an exit");
  DbRef destination = game_object_location(database, exit);

  return lua_mux_push_relationship(state, package, destination, "destination");
}

/**
 * Sets this exit's destination or clears it with `nil`.
 *
 * @par LuaLS definition mux callable Object:set_destination
 * @code{.lua}
 * ---Sets this exit's destination, or clears it when `destination` is nil.
 * ---@param destination DbRef|Object|nil Live object capable of containing objects, or nil to unlink this exit. This argument must be supplied explicitly.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `destination` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when the receiver is not an exit or the destination cannot contain objects, or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable) when the receiver or destination is being destroyed.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function Object:set_destination(destination) end
 * @endcode
 */
static int lua_mux_object_set_destination(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:set_destination");
  DbRef exit = lua_mux_require_object(package, state, 1);
  if (!is_exit(database, exit))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not an exit");
  if (is_going(database, exit))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "exit is being destroyed");
  if (lua_gettop(state) < 2)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "destination is required; pass nil to unlink");
  if (lua_isnil(state, 2)) {
    game_object_set_location(database, exit, NOTHING);
    return 0;
  }

  DbRef destination = lua_mux_require_object(package, state, 2);
  if (!has_contents(database, destination))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_INVALID,
                         "destination must be an object that can contain "
                         "objects");
  if (is_going(database, destination))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "destination is being destroyed");
  game_object_set_location(database, exit, destination);
  return 0;
}

/**
 * Returns this thing or player's home.
 *
 * @par LuaLS definition mux callable Object:home
 * @code{.lua}
 * ---Returns this thing or player's home, or nil when no home is assigned or the
 * ---home is being destroyed.
 * ---@return Object? home
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---during `@lua/check`, or
 * ---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
 * ---the receiver is not a thing or player or its stored home is invalid.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * function Object:home() end
 * @endcode
 */
static int lua_mux_object_home(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:home");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (!has_location(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not a thing or player");
  return lua_mux_push_relationship(state, package,
                                   game_object_link(database, object), "home");
}

/**
 * Sets this thing or player's home.
 *
 * @par LuaLS definition mux callable Object:set_home
 * @code{.lua}
 * ---Sets this thing or player's home to a live object capable of containing
 * ---objects.
 * ---@param new_home DbRef|Object Live room, thing, or player to assign as the object's home.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid)
 * ---when `new_home` is omitted,
 * ---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
 * ---the receiver is not a thing or player, the home cannot contain objects, or
 * ---the object would be its own home, or
 * ---[`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable)
 * ---when the receiver or home is being destroyed.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function Object:set_home(new_home) end
 * @endcode
 */
static int lua_mux_object_set_home(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:set_home");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (!has_location(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not a thing or player");
  if (is_going(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is being destroyed");
  if (lua_gettop(state) < 2)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "new_home is required");

  DbRef new_home = lua_mux_require_object(package, state, 2);
  if (!has_contents(database, new_home))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_INVALID,
                         "new_home must be an object that can contain objects");
  if (is_going(database, new_home))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "new_home is being destroyed");
  if (object == new_home)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot be its own home");

  game_object_set_link(database, object, new_home);
  return 0;
}

/**
 * Returns this thing or player's location.
 *
 * @par LuaLS definition mux callable Object:location
 * @code{.lua}
 * ---Returns this thing or player's current location, or nil when no location is
 * ---assigned or the location is being destroyed.
 * ---@return Object? location
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---during `@lua/check`, or
 * ---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) when
 * ---the receiver is not a thing or player or its stored location is invalid.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * function Object:location() end
 * @endcode
 */
static int lua_mux_object_location(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;

  lua_mux_require_runtime(package, state, "object:location");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (!has_location(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not a thing or player");
  return lua_mux_push_relationship(
      state, package, game_object_location(database, object), "location");
}

/**
 * Returns this object's assigned zone.
 *
 * @par LuaLS definition mux callable Object:zone
 * @code{.lua}
 * ---Returns this object's assigned zone, or nil when no zone is assigned or the zone is being destroyed.
 * ---@return Object? zone Assigned zone.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * function Object:zone() end
 * @endcode
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
 * @par LuaLS definition mux callable Object:set_zone
 * @code{.lua}
 * ---Assigns this object's zone, or clears it when `zone` is nil.
 * ---@param zone DbRef|Object|nil Live thing or room to assign, or nil to clear the zone. This argument must be supplied explicitly.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `zone` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function Object:set_zone(zone) end
 * @endcode
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
  game_object_set_zone(database, object, zone);
  return 0;
}

/**
 * Returns this object's assigned affiliation.
 *
 * @par LuaLS definition mux callable Object:affiliation
 * @code{.lua}
 * ---Returns this object's assigned affiliation, or nil when none is assigned or the affiliate is being destroyed.
 * ---@return Object? affiliation Assigned affiliation.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) for an invalid receiver or stored affiliation.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * function Object:affiliation() end
 * @endcode
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
 * @par LuaLS definition mux callable Object:set_affiliation
 * @code{.lua}
 * ---Assigns this object's affiliation, or clears it when `affiliation` is nil.
 * ---@param affiliation DbRef|Object|nil Any live object to assign, or nil to clear the affiliation. This argument must be supplied explicitly.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) during `@lua/check`, [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `affiliation` is omitted, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid) for an invalid reference, or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable) when either object is being destroyed.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function Object:set_affiliation(affiliation) end
 * @endcode
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
 * @par LuaLS definition mux callable Object:lua_parent
 * @code{.lua}
 * ---Returns this object's direct Lua parent path, or nil when none is assigned.
 * ---@return string? parent `object_logic`-relative parent path.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * function Object:lua_parent() end
 * @endcode
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
 * @par LuaLS definition mux callable Object:set_lua_parent
 * @code{.lua}
 * ---Assigns this object's direct Lua parent path, or clears it when `parent` is nil.
 * ---@param parent string|nil Existing `object_logic`-relative `.lua` path, or nil to clear it. This argument must be supplied explicitly.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) when `parent` is omitted or malformed, [`mux.error.codes.module.invalid`](lua://mux.error.codes.module.invalid) for an invalid or unavailable path, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.module.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function Object:set_lua_parent(parent) end
 * @endcode
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
  lua_pushcclosure(state, lua_mux_object_destination, 1);
  lua_setfield(state, -2, "destination");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_set_destination, 1);
  lua_setfield(state, -2, "set_destination");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_home, 1);
  lua_setfield(state, -2, "home");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_set_home, 1);
  lua_setfield(state, -2, "set_home");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_location, 1);
  lua_setfield(state, -2, "location");
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
