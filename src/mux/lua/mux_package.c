/* mux_package.c - Built-in Lua mux package bindings. */

#include "mux/server/platform.h"

#include "mux/lua/mux_package.h"

#include <lauxlib.h>

#include "mux/database/attrs.h"
#include "mux/database/flags.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/access.h"
#include "mux/world/object_spatial.h"

static LuaMuxPackage *lua_mux_package_get(lua_State *state) {
  return lua_touserdata(state, lua_upvalueindex(1));
}

static int lua_mux_package_is_checking(LuaMuxPackage *package) {
  return package->is_checking && package->is_checking(package->context);
}

static void lua_mux_require_runtime(LuaMuxPackage *package, lua_State *state,
                                    const char *function) {
  if (lua_mux_package_is_checking(package))
    luaL_error(state, "mux.%s is unavailable during @lua/check", function);
}

static DbRef lua_mux_require_object(LuaMuxPackage *package, lua_State *state,
                                    int argument) {
  DbRef object = (DbRef)luaL_checkinteger(state, argument);

  if (!is_good_obj(package->services->database, object))
    luaL_argerror(state, argument, "invalid object");
  return object;
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
    return luaL_argerror(state, 1, "object cannot contain other objects");
  lua_newtable(state);
  DOLIST(package->services->database, member,
         game_object_contents(package->services->database, object)) {
    lua_pushinteger(state, member);
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
    return luaL_argerror(state, 1, "object cannot contain other objects");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_contents(package->services->database, container), member))
    return luaL_argerror(state, 3, "object is not directly contained");
  evaluation = &package->services->background_command->evaluation;
  can_see_location = !is_dark(package->services->database, container);
  lua_pushboolean(state, can_see(evaluation, package->services->configuration,
                                 viewer, member, can_see_location));
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
    return luaL_argerror(state, 1, "object cannot have exits");
  lua_newtable(state);
  DOLIST(package->services->database, exit,
         game_object_exits(package->services->database, object)) {
    lua_pushinteger(state, exit);
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
    return luaL_argerror(state, 1, "object cannot have exits");
  if (!is_exit(package->services->database, exit))
    return luaL_argerror(state, 3, "object is not an exit");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_exits(package->services->database, location), exit))
    return luaL_argerror(state, 3, "exit is not directly attached");
  if (is_dark(package->services->database, location))
    key |= VE_LOC_DARK;
  lua_pushboolean(
      state, exit_displayable(package->services->database, exit, viewer, key));
  return 1;
}

static int lua_mux_object_name(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;

  lua_mux_require_runtime(package, state, "object_name");
  object = lua_mux_require_object(package, state, 1);
  lua_pushstring(state, game_object_name(package->services->database, object));
  return 1;
}

static int lua_mux_object_description(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  const char *description;

  lua_mux_require_runtime(package, state, "object_description");
  object = lua_mux_require_object(package, state, 1);
  description = attribute_get_raw(package->services->database, object, A_DESC);
  if (description)
    lua_pushstring(state, description);
  else
    lua_pushnil(state);
  return 1;
}

static int lua_mux_object_type(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  const char *type;

  lua_mux_require_runtime(package, state, "object_type");
  object = lua_mux_require_object(package, state, 1);
  switch (typeof_obj(package->services->database, object)) {
  case TYPE_ROOM:
    type = "room";
    break;
  case TYPE_THING:
    type = "thing";
    break;
  case TYPE_EXIT:
    type = "exit";
    break;
  case TYPE_PLAYER:
    type = "player";
    break;
  default:
    return luaL_error(state, "invalid object type");
  }
  lua_pushstring(state, type);
  return 1;
}

static int lua_mux_attr_get(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object = (DbRef)luaL_checkinteger(state, 1);
  const char *name = luaL_checkstring(state, 2);
  const char *value;

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.attr_get is unavailable during @lua/check");
  if (!is_good_obj(package->services->database, object))
    return luaL_error(state, "invalid object");
  value = dynamic_attribute_get(package->services->database, object, name);
  if (!value)
    lua_pushnil(state);
  else
    lua_pushstring(state, value);
  return 1;
}

static int lua_mux_attr_set(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object = (DbRef)luaL_checkinteger(state, 1);
  const char *name = luaL_checkstring(state, 2);
  const char *value = luaL_checkstring(state, 3);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.attr_set is unavailable during @lua/check");
  if (!is_good_obj(package->services->database, object))
    return luaL_error(state, "invalid object");
  if (!dynamic_attribute_set(package->services->database, object, name, value))
    return luaL_error(state, "invalid attribute");
  return 0;
}

static int lua_mux_notify(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object = (DbRef)luaL_checkinteger(state, 1);
  const char *message = luaL_checkstring(state, 2);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.notify is unavailable during @lua/check");
  if (!is_good_obj(package->services->database, object))
    return luaL_error(state, "invalid object");
  notify(&package->services->background_command->evaluation, object, message);
  return 0;
}

static int lua_mux_connected_players(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor;
  DescriptorIterator iterator =
      descriptor_iterator_connected(package->services->descriptors);
  int index = 1;

  lua_newtable(state);
  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    lua_newtable(state);
    lua_pushstring(state, game_object_name(package->services->database,
                                           descriptor->player));
    lua_setfield(state, -2, "name");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->connected_at));
    lua_setfield(state, -2, "connected_for");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->last_time));
    lua_setfield(state, -2, "idle_for");
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

static int lua_mux_who_summary(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_newtable(state);
  lua_pushinteger(state, 0);
  lua_setfield(state, -2, "hidden");
  lua_pushinteger(state, *package->services->record_players);
  lua_setfield(state, -2, "record");
  if (package->services->configuration->max_players == -1)
    lua_pushnil(state);
  else
    lua_pushinteger(state, package->services->configuration->max_players);
  lua_setfield(state, -2, "maximum");
  return 1;
}

static int lua_mux_flow_start(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  int descriptor_id = (int)luaL_checkinteger(state, 1);
  const char *module = luaL_checkstring(state, 2);
  const char *first_step = luaL_checkstring(state, 3);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.flow_start is unavailable during @lua/check");
  if (!package->flow_start)
    return luaL_error(state, "mux.flow_start is unavailable");
  return package->flow_start(package->context, state, descriptor_id, module,
                             first_step);
}

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
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
  lua_pushcclosure(state, lua_mux_object_description, 1);
  lua_setfield(state, -2, "object_description");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_name, 1);
  lua_setfield(state, -2, "object_name");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_type, 1);
  lua_setfield(state, -2, "object_type");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_attr_get, 1);
  lua_setfield(state, -2, "attr_get");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_attr_set, 1);
  lua_setfield(state, -2, "attr_set");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_notify, 1);
  lua_setfield(state, -2, "notify");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_connected_players, 1);
  lua_setfield(state, -2, "connected_players");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_who_summary, 1);
  lua_setfield(state, -2, "who_summary");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_flow_start, 1);
  lua_setfield(state, -2, "flow_start");
  lua_setglobal(state, "mux");
}
