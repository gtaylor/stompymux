/* mux_package.c - Built-in Lua mux package bindings. */

#include <lauxlib.h>
#include <lua.h>
#include <math.h>
#include <stdint.h>
#include <string.h>

#include "mux/lua/mux_package.h"
#include "mux/lua/mux_package_internal.h"
#include "mux/objects/object_state.h"
#include "mux/support/checked_storage.h"

static int lua_mux_object_state(lua_State *state) {
  LuaMuxObject *object = lua_mux_check_object_handle(state, 1);
  size_t length;
  const char *name_space = luaL_checklstring(state, 2, &length);
  LuaMuxState *handle;

  lua_mux_require_runtime(object->package, state, "object:state");
  if (length >= sizeof(handle->name_space) ||
      memchr(name_space, '\0', length) ||
      !object_state_name_is_valid(name_space))
    return luaL_argerror(state, 2, "invalid state namespace");
  handle = lua_newuserdata(state, sizeof(*handle));
  *handle = (LuaMuxState){
      .package = object->package,
      .object = object->object,
      .generation = object->generation,
  };
  memcpy(handle->name_space, name_space, length);
  *(char *)checked_storage_at(handle->name_space, sizeof(handle->name_space),
                              sizeof(char), length) = '\0';
  luaL_getmetatable(state, LUA_MUX_STATE_METATABLE);
  lua_setmetatable(state, -2);
  return 1;
}

static void lua_mux_push_state_value(lua_State *state,
                                     const ObjectStateValue *value) {
  switch (value->type) {
  case OBJECT_STATE_STRING:
    lua_pushlstring(state, value->as.string.data, value->as.string.length);
    break;
  case OBJECT_STATE_BOOLEAN:
    lua_pushboolean(state, value->as.boolean);
    break;
  case OBJECT_STATE_INTEGER:
    lua_pushinteger(state, (lua_Integer)value->as.integer);
    break;
  case OBJECT_STATE_NUMBER:
    lua_pushnumber(state, (lua_Number)value->as.number);
    break;
  }
}

static const char *lua_mux_state_key(lua_State *state, int argument) {
  size_t length;
  const char *key = luaL_checklstring(state, argument, &length);

  if (length > 255 || memchr(key, '\0', length) ||
      !object_state_name_is_valid(key))
    luaL_argerror(state, argument, "invalid state key");
  return key;
}

static bool lua_mux_read_state_value(lua_State *state, int argument,
                                     ObjectStateValue *value) {
  memset(value, 0, sizeof(*value));
  switch (lua_type(state, argument)) {
  case LUA_TSTRING:
    value->type = OBJECT_STATE_STRING;
    value->as.string.data =
        lua_tolstring(state, argument, &value->as.string.length);
    return true;
  case LUA_TBOOLEAN:
    value->type = OBJECT_STATE_BOOLEAN;
    value->as.boolean = (lua_toboolean(state, argument) != 0);
    return true;
  case LUA_TNUMBER: {
    lua_Number number = lua_tonumber(state, argument);
    lua_Integer integer = lua_tointeger(state, argument);

    if (!isfinite((double)number))
      return false;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wfloat-equal"
    if ((lua_Number)integer == number) {
#pragma clang diagnostic pop
      value->type = OBJECT_STATE_INTEGER;
      value->as.integer = (int64_t)integer;
    } else {
      value->type = OBJECT_STATE_NUMBER;
      value->as.number = (double)number;
    }
    return true;
  }
  default:
    return false;
  }
}

static int lua_mux_state_get(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  const ObjectStateValue *value =
      transaction->depth
          ? object_state_transaction_get(transaction, handle->object,
                                         handle->name_space, key)
          : object_state_get(handle->package->services->database,
                             handle->object, handle->name_space, key);

  if (value)
    lua_mux_push_state_value(state, value);
  else if (lua_gettop(state) >= 3)
    lua_pushvalue(state, 3);
  else
    lua_pushnil(state);
  return 1;
}

static int lua_mux_state_has(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;

  lua_pushboolean(
      state, (transaction->depth
                  ? object_state_transaction_get(transaction, handle->object,
                                                 handle->name_space, key)
                  : object_state_get(handle->package->services->database,
                                     handle->object, handle->name_space,
                                     key)) != nullptr);
  return 1;
}

static int lua_mux_state_set(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateValue value;
  char error[256];

  if (lua_isnil(state, 3)) {
    object_state_transaction_delete(&handle->package->state_transaction,
                                    handle->object, handle->name_space, key);
    return 0;
  }
  if (!lua_mux_read_state_value(state, 3, &value))
    return luaL_argerror(
        state, 3, "state values must be strings, booleans, or finite numbers");
  if (!object_state_transaction_set(&handle->package->state_transaction,
                                    handle->object, handle->name_space, key,
                                    &value, error, sizeof(error)))
    return luaL_error(state, "%s", error);
  return 0;
}

static int lua_mux_state_delete(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  bool existed =
      (transaction->depth
           ? object_state_transaction_get(transaction, handle->object,
                                          handle->name_space, key)
           : object_state_get(handle->package->services->database,
                              handle->object, handle->name_space, key)) !=
      nullptr;

  if (existed)
    object_state_transaction_delete(&handle->package->state_transaction,
                                    handle->object, handle->name_space, key);
  lua_pushboolean(state, existed);
  return 1;
}

static int lua_mux_state_keys(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  size_t count;

  if (!transaction->depth)
    return luaL_error(state, "state enumeration requires an active callback");
  count = object_state_transaction_count(transaction, handle->object,
                                         handle->name_space);

  lua_createtable(state, (int)count, 0);
  for (size_t index = 0; index < count; index++) {
    ObjectStateEntryView entry;

    if (!object_state_transaction_entry(transaction, handle->object,
                                        handle->name_space, index, &entry))
      return luaL_error(state, "state changed during enumeration");
    lua_pushstring(state, entry.key);
    lua_rawseti(state, -2, (int)index + 1);
  }
  return 1;
}

static int lua_mux_state_entries(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  size_t count;

  if (!transaction->depth)
    return luaL_error(state, "state enumeration requires an active callback");
  count = object_state_transaction_count(transaction, handle->object,
                                         handle->name_space);

  lua_createtable(state, (int)count, 0);
  for (size_t index = 0; index < count; index++) {
    ObjectStateEntryView entry;

    if (!object_state_transaction_entry(transaction, handle->object,
                                        handle->name_space, index, &entry))
      return luaL_error(state, "state changed during enumeration");
    lua_createtable(state, 0, 2);
    lua_pushstring(state, entry.key);
    lua_setfield(state, -2, "key");
    lua_mux_push_state_value(state, entry.value);
    lua_setfield(state, -2, "value");
    lua_rawseti(state, -2, (int)index + 1);
  }
  return 1;
}

static int lua_mux_state_get_many(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  size_t count;

  luaL_checktype(state, 2, LUA_TTABLE);
  count = lua_objlen(state, 2);
  lua_createtable(state, 0, (int)count);
  for (size_t index = 1; index <= count; index++) {
    const char *key;
    const ObjectStateValue *value;

    lua_rawgeti(state, 2, (int)index);
    key = lua_mux_state_key(state, -1);
    ObjectStateTransaction *transaction = &handle->package->state_transaction;
    value = transaction->depth
                ? object_state_transaction_get(transaction, handle->object,
                                               handle->name_space, key)
                : object_state_get(handle->package->services->database,
                                   handle->object, handle->name_space, key);
    if (value) {
      lua_mux_push_state_value(state, value);
      lua_setfield(state, -3, key);
    }
    lua_pop(state, 1);
  }
  return 1;
}

static int lua_mux_state_set_many(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);

  luaL_checktype(state, 2, LUA_TTABLE);
  lua_pushnil(state);
  while (lua_next(state, 2) != 0) {
    ObjectStateValue value;
    const char *key;
    char error[256];

    if (lua_type(state, -2) != LUA_TSTRING)
      return luaL_error(state, "state update keys must be strings");
    key = lua_mux_state_key(state, -2);
    if (lua_isnil(state, -1)) {
      object_state_transaction_delete(&handle->package->state_transaction,
                                      handle->object, handle->name_space, key);
    } else {
      if (!lua_mux_read_state_value(state, -1, &value))
        return luaL_error(
            state, "state values must be strings, booleans, or finite numbers");
      if (!object_state_transaction_set(&handle->package->state_transaction,
                                        handle->object, handle->name_space, key,
                                        &value, error, sizeof(error)))
        return luaL_error(state, "%s", error);
    }
    lua_pop(state, 1);
  }
  return 0;
}

static int lua_mux_state_tostring(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);

  lua_pushfstring(state, "state(#%d, %s)", (int)handle->object,
                  handle->name_space);
  return 1;
}

void lua_mux_install_state_bindings(lua_State *state,
                                    LuaMuxPackage *package [[maybe_unused]]) {
  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_pushcfunction(state, lua_mux_object_state);
  lua_setfield(state, -2, "state");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_STATE_METATABLE);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_state_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_state_get);
  lua_setfield(state, -2, "get");
  lua_pushcfunction(state, lua_mux_state_has);
  lua_setfield(state, -2, "has");
  lua_pushcfunction(state, lua_mux_state_set);
  lua_setfield(state, -2, "set");
  lua_pushcfunction(state, lua_mux_state_delete);
  lua_setfield(state, -2, "delete");
  lua_pushcfunction(state, lua_mux_state_keys);
  lua_setfield(state, -2, "keys");
  lua_pushcfunction(state, lua_mux_state_entries);
  lua_setfield(state, -2, "entries");
  lua_pushcfunction(state, lua_mux_state_get_many);
  lua_setfield(state, -2, "get_many");
  lua_pushcfunction(state, lua_mux_state_set_many);
  lua_setfield(state, -2, "set_many");
  lua_pop(state, 1);
}
