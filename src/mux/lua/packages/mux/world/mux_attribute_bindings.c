/* mux_attribute_bindings.c - Lua bindings for native object attributes. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/commands/command_keys.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/object_set.h"

static const Attribute *lua_mux_attribute_name(lua_State *state,
                                               LuaMuxAttribute *handle,
                                               int argument) {
  size_t length;
  const char *name = luaL_checklstring(state, argument, &length);

  if (memchr(name, '\0', length))
    lua_error_arg(state, argument, LUA_ERROR_CODE_ATTRIBUTE_INVALID,
                  "invalid attribute name");
  const Attribute *attribute = object_attribute_administrable_by_name(
      handle->package->services->database, name);
  if (!attribute)
    lua_error_arg(state, argument, LUA_ERROR_CODE_ATTRIBUTE_INVALID,
                  "attribute is not administrable");
  return attribute;
}

/**
 * Creates an attribute handle for this object.
 *
 * @par Lua name `object:attributes`
 * @par Lua signature `object:attributes( )`
 * @par Lua parameters - None.
 * @par Lua returns - `attributes` (`Attribute`): A handle for the object's
 * supported native attributes.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object_attributes(lua_State *state) {
  LuaMuxObject *object = lua_mux_check_object_handle(state, 1);
  LuaMuxAttribute *handle;

  lua_mux_require_runtime(object->package, state, "object:attributes");
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

/**
 * Gets a supported native attribute.
 *
 * @par Lua name `attributes:get`
 * @par Lua signature `attributes:get( name )`
 * @par Lua parameters - `name` (`string`) A supported native attribute name.
 * @par Lua returns - `value` (`string|nil`): The raw value, or nil when unset.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Attribute;
 * `LUA_ERROR_CODE_ATTRIBUTE_INVALID` for an invalid or unsupported name.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_attribute_get(lua_State *state) {
  LuaMuxAttribute *handle = lua_mux_check_attribute(state, 1);
  const Attribute *attribute = lua_mux_attribute_name(state, handle, 2);
  const char *value = attribute_get_raw(handle->package->services->database,
                                        handle->object, attribute->number);

  if (value)
    lua_pushstring(state, value);
  else
    lua_pushnil(state);
  return 1;
}

/**
 * Sets or clears a supported native attribute.
 *
 * @par Lua name `attributes:set`
 * @par Lua signature `attributes:set( name, value )`
 * @par Lua parameters - `name` (`string`) A supported native attribute name.
 * - `value` (`string|nil`) The new raw value; nil clears the attribute.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Attribute;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ATTRIBUTE_INVALID` for an invalid name/value or a failed
 * native update.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_attribute_set(lua_State *state) {
  LuaMuxAttribute *handle = lua_mux_check_attribute(state, 1);
  const Attribute *attribute = lua_mux_attribute_name(state, handle, 2);
  char *text = alloc_lbuf("lua_mux_attribute_set");
  bool set;

  lua_mux_require_runtime(handle->package, state, "attribute:set");
  if (lua_isnil(state, 3)) {
    text[0] = '\0';
  } else {
    size_t length;
    const char *value = luaL_checklstring(state, 3, &length);

    if (length >= LBUF_SIZE || memchr(value, '\0', length)) {
      free_buf(text);
      return lua_error_arg(state, 3, LUA_ERROR_CODE_ATTRIBUTE_INVALID,
                           "invalid attribute value");
    }
    memcpy(text, value, length);
    *(char *)checked_storage_at(text, LBUF_SIZE, sizeof(char), length) = '\0';
  }
  set = object_attribute_set(
      &handle->package->services->background_command->evaluation, GOD,
      handle->object, attribute->number, text, SET_QUIET);
  free_buf(text);
  if (!set)
    return lua_error_raise(state, LUA_ERROR_CODE_ATTRIBUTE_INVALID,
                           "unable to set attribute");
  return 0;
}

/**
 * Returns every supported native attribute and its current value.
 *
 * @par Lua name `attributes:entries`
 * @par Lua signature `attributes:entries( )`
 * @par Lua parameters - None.
 * @par Lua returns - `entries` (`table`): A name-to-string table of all
 * supported attributes.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Attribute.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_attribute_entries(lua_State *state) {
  LuaMuxAttribute *handle = lua_mux_check_attribute(state, 1);
  GameDatabase *database = handle->package->services->database;

  lua_newtable(state);
  for (size_t index = 0; index < native_attribute_count(); index++) {
    const Attribute *attribute = native_attribute_at(index);

    const char *value;

    if (!object_attribute_is_administrable(attribute->number))
      continue;
    value = attribute_get_raw(database, handle->object, attribute->number);
    lua_pushstring(state, value ? value : "");
    lua_setfield(state, -2, attribute->name);
  }
  return 1;
}

void lua_mux_install_attribute_bindings(lua_State *state, LuaMuxPackage *package
                                        [[maybe_unused]]) {
  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_pushcfunction(state, lua_mux_object_attributes);
  lua_setfield(state, -2, "attributes");
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
