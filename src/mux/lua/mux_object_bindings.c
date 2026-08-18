/* mux_package.c - Built-in Lua mux package bindings. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/mux_package.h"
#include "mux/lua/mux_package_internal.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/world/access.h"
#include "mux/world/object_spatial.h"

DbRef lua_mux_require_object(LuaMuxPackage *package, lua_State *state,
                             int argument) {
  DbRef object;
  LuaMuxObject *handle =
      luaL_testudata(state, argument, LUA_MUX_OBJECT_METATABLE);

  if (handle) {
    if (handle->package != package)
      lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                    "object belongs to another Lua runtime");
    if (!is_good_obj(package->services->database, handle->object) ||
        game_object_generation(package->services->database, handle->object) !=
            handle->generation)
      lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                    "object no longer exists");
    object = handle->object;
  } else {
    object = (DbRef)luaL_checkinteger(state, argument);
  }

  if (!is_good_obj(package->services->database, object))
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "invalid object");
  return object;
}

LuaMuxObject *lua_mux_push_object(lua_State *state, LuaMuxPackage *package,
                                  DbRef object) {
  LuaMuxObject *handle = lua_newuserdata(state, sizeof(*handle));

  *handle = (LuaMuxObject){
      .package = package,
      .object = object,
      .generation = game_object_generation(package->services->database, object),
  };
  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_setmetatable(state, -2);
  return handle;
}

LuaMuxObject *lua_mux_check_object_handle(lua_State *state, int argument) {
  LuaMuxObject *handle =
      luaL_checkudata(state, argument, LUA_MUX_OBJECT_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "object no longer exists");
  return handle;
}

LuaMuxState *lua_mux_check_state(lua_State *state, int argument) {
  LuaMuxState *handle =
      luaL_checkudata(state, argument, LUA_MUX_STATE_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "object no longer exists");
  return handle;
}

LuaMuxAttribute *lua_mux_check_attribute(lua_State *state, int argument) {
  LuaMuxAttribute *handle =
      luaL_checkudata(state, argument, LUA_MUX_ATTRIBUTE_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "object no longer exists");
  return handle;
}

static int lua_mux_object(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;

  lua_mux_require_runtime(package, state, "object");
  object = lua_mux_require_object(package, state, 1);
  lua_mux_push_object(state, package, object);
  return 1;
}

static bool lua_mux_list_contains(GameDatabase *database, DbRef first,
                                  DbRef member) {
  DbRef object;

  DOLIST(database, object, first) {
    if (object == member)
      return true;
  }
  return false;
}

static int lua_mux_contents(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  DbRef member;
  int index = 1;

  lua_mux_require_runtime(package, state, "contents");
  object = lua_mux_require_object(package, state, 1);
  if (!has_contents(package->services->database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot contain other objects");
  lua_newtable(state);
  DOLIST(package->services->database, member,
         game_object_contents(package->services->database, object)) {
    lua_mux_push_object(state, package, member);
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

static int lua_mux_contents_visible(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  EvaluationContext *evaluation;
  DbRef container;
  DbRef viewer;
  DbRef member;
  bool can_see_location;

  lua_mux_require_runtime(package, state, "contents_visible");
  container = lua_mux_require_object(package, state, 1);
  viewer = lua_mux_require_object(package, state, 2);
  member = lua_mux_require_object(package, state, 3);
  if (!has_contents(package->services->database, container))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot contain other objects");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_contents(package->services->database, container), member))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not directly contained");
  evaluation = &package->services->background_command->evaluation;
  can_see_location = ((!is_dark(package->services->database, container)) != 0);
  lua_pushboolean(state, can_see(&(ObjectVisibilityRequest){
                             .evaluation = evaluation,
                             .viewer = viewer,
                             .object = member,
                             .location_visible = can_see_location}));
  return 1;
}

static int lua_mux_exits(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  DbRef exit;
  int index = 1;

  lua_mux_require_runtime(package, state, "exits");
  object = lua_mux_require_object(package, state, 1);
  if (!has_exits(package->services->database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot have exits");
  lua_newtable(state);
  DOLIST(package->services->database, exit,
         game_object_exits(package->services->database, object)) {
    lua_mux_push_object(state, package, exit);
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

static int lua_mux_exits_visible(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef location;
  DbRef viewer;
  DbRef exit;
  int key = 0;

  lua_mux_require_runtime(package, state, "exits_visible");
  location = lua_mux_require_object(package, state, 1);
  viewer = lua_mux_require_object(package, state, 2);
  exit = lua_mux_require_object(package, state, 3);
  if (!has_exits(package->services->database, location))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot have exits");
  if (!is_exit(package->services->database, exit))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not an exit");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_exits(package->services->database, location), exit))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_OBJECT_INVALID,
                         "exit is not directly attached");
  if (is_dark(package->services->database, location))
    key |= VE_LOC_DARK;
  lua_pushboolean(state, exit_displayable(&(ExitVisibilityRequest){
                             .database = package->services->database,
                             .exit = exit,
                             .viewer = viewer,
                             .options = key}));
  return 1;
}

static int lua_mux_exit_enter_lock_passes(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef exit;
  DbRef enactor;

  lua_mux_require_runtime(package, state, "exit_enter_lock_passes");
  exit = lua_mux_require_object(package, state, 1);
  enactor = lua_mux_require_object(package, state, 2);
  if (!is_exit(package->services->database, exit))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not an exit");
  if (!package->exit_enter_lock_passes)
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                           "mux.exit_enter_lock_passes is unavailable");
  lua_pushboolean(
      state, package->exit_enter_lock_passes(package->context, exit, enactor));
  return 1;
}

static int lua_mux_object_index(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);
  LuaMuxPackage *package = handle->package;
  const char *key = luaL_checkstring(state, 2);
  GameDatabase *database = package->services->database;

  if (!strcmp(key, "dbref")) {
    lua_pushinteger(state, handle->object);
    return 1;
  }
  if (!strcmp(key, "name")) {
    lua_pushstring(state, game_object_name(database, handle->object));
    return 1;
  }
  if (!strcmp(key, "type")) {
    switch (typeof_obj(database, handle->object)) {
    case OBJECT_TYPE_ROOM:
      lua_pushliteral(state, "room");
      break;
    case OBJECT_TYPE_THING:
      lua_pushliteral(state, "thing");
      break;
    case OBJECT_TYPE_EXIT:
      lua_pushliteral(state, "exit");
      break;
    case OBJECT_TYPE_PLAYER:
      lua_pushliteral(state, "player");
      break;
    default:
      lua_pushnil(state);
      break;
    }
    return 1;
  }
  if (!strcmp(key, "description") || !strcmp(key, "inside_description")) {
    int attribute = !strcmp(key, "description") ? A_DESC : A_IDESC;
    const char *description =
        attribute_get_raw(database, handle->object, attribute);
    if (description)
      lua_pushstring(state, description);
    else
      lua_pushnil(state);
    return 1;
  }
  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_getfield(state, -1, key);
  lua_remove(state, -2);
  return 1;
}

static int lua_mux_object_tostring(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);

  lua_pushfstring(state, "object(#%d)", (int)handle->object);
  return 1;
}

static int lua_mux_object_equal(lua_State *state) {
  LuaMuxObject *left = luaL_checkudata(state, 1, LUA_MUX_OBJECT_METATABLE);
  LuaMuxObject *right = luaL_checkudata(state, 2, LUA_MUX_OBJECT_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->object == right->object &&
                             left->generation == right->generation);
  return 1;
}

void lua_mux_install_object_bindings(lua_State *state, LuaMuxPackage *package) {
  luaL_newmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_pushcfunction(state, lua_mux_object_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_object_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_object_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_contents, 1);
  lua_setfield(state, -2, "contents");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_contents_visible, 1);
  lua_setfield(state, -2, "contents_visible");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_exits, 1);
  lua_setfield(state, -2, "exits");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_exits_visible, 1);
  lua_setfield(state, -2, "exits_visible");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_exit_enter_lock_passes, 1);
  lua_setfield(state, -2, "enter_lock_passes");
  lua_pop(state, 1);

  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object, 1);
  lua_setfield(state, -2, "object");
}
