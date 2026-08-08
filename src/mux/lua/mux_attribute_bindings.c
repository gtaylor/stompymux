#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/commands/command_keys.h"
#include "mux/lua/mux_package.h"
#include "mux/lua/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/object_set.h"

static Attribute *lua_mux_attribute_name(lua_State *state,
                                         LuaMuxAttribute *handle,
                                         int argument) {
  size_t length;
  const char *name = luaL_checklstring(state, argument, &length);

  if (memchr(name, '\0', length))
    luaL_argerror(state, argument, "invalid attribute name");
  Attribute *attribute = object_attribute_administrable_by_name(
      handle->package->services->database, name);
  if (!attribute)
    luaL_argerror(state, argument, "attribute is not administrable");
  return attribute;
}

static int lua_mux_object_attribute(lua_State *state) {
  LuaMuxObject *object = lua_mux_check_object_handle(state, 1);
  LuaMuxAttribute *handle;

  lua_mux_require_runtime(object->package, state, "object:attribute");
  handle = lua_newuserdata(state, sizeof(*handle));
  *handle = (LuaMuxAttribute){
      .package = object->package,
      .object = object->object,
      .generation = object->generation,
  };
  luaL_getmetatable(state, LUA_MUX_ATTRIBUTE_METATABLE);
  lua_setmetatable(state, -2);
  return 1;
}

static int lua_mux_attribute_get(lua_State *state) {
  LuaMuxAttribute *handle = lua_mux_check_attribute(state, 1);
  Attribute *attribute = lua_mux_attribute_name(state, handle, 2);
  const char *value = attribute_get_raw(handle->package->services->database,
                                        handle->object, attribute->number);

  if (value)
    lua_pushstring(state, value);
  else
    lua_pushnil(state);
  return 1;
}

static int lua_mux_attribute_set(lua_State *state) {
  LuaMuxAttribute *handle = lua_mux_check_attribute(state, 1);
  Attribute *attribute = lua_mux_attribute_name(state, handle, 2);
  char *text = alloc_lbuf("lua_mux_attribute_set");
  bool set;

  lua_mux_require_runtime(handle->package, state, "attribute:set");
  if (lua_isnil(state, 3)) {
    text[0] = '\0';
  } else {
    size_t length;
    const char *value = luaL_checklstring(state, 3, &length);

    if (length >= LBUF_SIZE || memchr(value, '\0', length)) {
      free_lbuf(text);
      return luaL_argerror(state, 3, "invalid attribute value");
    }
    memcpy(text, value, length);
    *(char *)checked_storage_at(text, LBUF_SIZE, sizeof(char), length) = '\0';
  }
  set = object_attribute_set(
      &handle->package->services->background_command->evaluation, GOD,
      handle->object, attribute->number, text, SET_QUIET);
  free_lbuf(text);
  if (!set)
    return luaL_error(state, "unable to set attribute");
  return 0;
}

static int lua_mux_attribute_entries(lua_State *state) {
  LuaMuxAttribute *handle = lua_mux_check_attribute(state, 1);
  GameDatabase *database = handle->package->services->database;

  lua_newtable(state);
  for (size_t index = 0; index < native_attribute_count(); index++) {
    Attribute *attribute = native_attribute_at(index);

    const char *value;

    if (!object_attribute_is_administrable(attribute->number))
      continue;
    value = attribute_get_raw(database, handle->object, attribute->number);
    lua_pushstring(state, value ? value : "");
    lua_setfield(state, -2, attribute->name);
  }
  return 1;
}

void lua_mux_install_attribute_bindings(lua_State *state,
                                        LuaMuxPackage *package) {
  (void)package;
  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_pushcfunction(state, lua_mux_object_attribute);
  lua_setfield(state, -2, "attribute");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_ATTRIBUTE_METATABLE);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_attribute_get);
  lua_setfield(state, -2, "get");
  lua_pushcfunction(state, lua_mux_attribute_set);
  lua_setfield(state, -2, "set");
  lua_pushcfunction(state, lua_mux_attribute_entries);
  lua_setfield(state, -2, "entries");
  lua_pop(state, 1);
}
