/* lua_error.c - Structured Lua error implementation. */

#include "mux/lua/lua_error.h"

#include <lauxlib.h>
#include <lua.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "mux/lua/lua_error_codes.h"
#include "mux/support/checked_storage.h"

// NOLINTBEGIN(bugprone-easily-swappable-parameters): Lua error APIs mirror Lua.

const char LUA_ERROR_METATABLE[] = "stompymux.error";
const char LUA_TRACEBACK_KEY[] = "btmux.lua.traceback";

static const char LUA_ERROR_CODE_NODE_METATABLE[] = "stompymux.error_code";
static const char LUA_ERROR_CODE_TREES_KEY[] = "btmux.lua.error_code_trees";

const char *const LUA_ERROR_CODE_NAMES[LUA_ERROR_CODE_COUNT] = {
    [LUA_ERROR_CODE_ARG_INVALID] = "mux.arg.invalid",
    [LUA_ERROR_CODE_CHECKING_UNAVAILABLE] = "mux.unavailable.checking",
    [LUA_ERROR_CODE_RUNTIME] = "mux.runtime",
    [LUA_ERROR_CODE_STATE_INVALID] = "mux.state.invalid",
    [LUA_ERROR_CODE_STATE_VALUE_TOO_LARGE] = "mux.state.value_too_large",
    [LUA_ERROR_CODE_STATE_UNAVAILABLE] = "mux.state.unavailable",
    [LUA_ERROR_CODE_OBJECT_INVALID] = "mux.object.invalid",
    [LUA_ERROR_CODE_OBJECT_UNAVAILABLE] = "mux.object.unavailable",
    [LUA_ERROR_CODE_ATTRIBUTE_INVALID] = "mux.attribute.invalid",
    [LUA_ERROR_CODE_FLAG_INVALID] = "mux.flag.invalid",
    [LUA_ERROR_CODE_POWER_INVALID] = "mux.power.invalid",
    [LUA_ERROR_CODE_ACCESS_INVALID] = "mux.access.invalid",
    [LUA_ERROR_CODE_CONNECTION_INVALID] = "mux.connection.invalid",
    [LUA_ERROR_CODE_CONNECTION_UNAVAILABLE] = "mux.connection.unavailable",
    [LUA_ERROR_CODE_CHANNEL_INVALID] = "mux.channel.invalid",
    [LUA_ERROR_CODE_CHANNEL_FLAG_INVALID] = "mux.channel_flag.invalid",
    [LUA_ERROR_CODE_TEXT_INVALID] = "mux.text.invalid",
    [LUA_ERROR_CODE_MODULE_INVALID] = "mux.module.invalid",
    [LUA_ERROR_CODE_MODULE_UNAVAILABLE] = "mux.module.unavailable",
    [LUA_ERROR_CODE_CONFIG_NOT_FOUND] = "mux.config.not_found",
    [LUA_ERROR_CODE_CONFIG_UNSUPPORTED] = "mux.config.unsupported",
    [LUA_ERROR_CODE_BTECH_UNAVAILABLE] = "btech.unavailable",
    [LUA_ERROR_CODE_BTECH_FAILED] = "btech.failed",
    [LUA_ERROR_CODE_TESTING_ASSERTION] = "testing.assertion",
    [LUA_ERROR_CODE_TESTING_RUNTIME] = "testing.runtime",
    [LUA_ERROR_CODE_INTERNAL] = "mux.internal",
};

static void lua_error_copy(char *out, size_t out_size, const char *value) {
  if (!out || !out_size)
    return;
  (void)snprintf(out, out_size, "%s", value ? value : "");
}

static bool lua_error_code_matches(const char *actual, const char *wanted) {
  size_t actual_size;
  size_t size;

  if (!actual || !wanted)
    return false;
  actual_size = strlen(actual);
  size = strlen(wanted);
  if (actual_size < size || strncmp(actual, wanted, size) != 0)
    return false;
  if (actual_size == size)
    return true;
  return *(const char *)checked_storage_at_const(actual, actual_size,
                                                 sizeof(char), size) == '.';
}

const char *lua_error_code_name(LuaErrorCode code) {
  if (code < 0 || code >= LUA_ERROR_CODE_COUNT)
    code = LUA_ERROR_CODE_INTERNAL;
  return *(const char *const *)checked_storage_at_const(
      (const void *)LUA_ERROR_CODE_NAMES, LUA_ERROR_CODE_COUNT,
      sizeof(*LUA_ERROR_CODE_NAMES), (size_t)code);
}

static char lua_error_code_character_at(const char *code, size_t code_size,
                                        size_t index) {
  return *(const char *)checked_storage_at_const(code, code_size, sizeof(char),
                                                 index);
}

static const char *lua_error_code_at(const char *code, size_t code_size,
                                     size_t index) {
  return checked_storage_at_const(code, code_size, sizeof(char), index);
}

static int lua_error_code_node_tostring(lua_State *state) {
  lua_getfield(state, 1, "code");
  return 1;
}

static int lua_error_code_node_index(lua_State *state) {
  const char *key = lua_tostring(state, 2);

  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "unknown Lua error code segment '%s'",
                         key ? key : "<non-string>");
}

static int lua_error_code_node_newindex(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "Lua error code nodes are immutable");
}

static void lua_error_install_code_node_metatable(lua_State *state) {
  if (luaL_newmetatable(state, LUA_ERROR_CODE_NODE_METATABLE)) {
    lua_pushcfunction(state, lua_error_code_node_tostring);
    lua_setfield(state, -2, "__tostring");
    lua_pushcfunction(state, lua_error_code_node_index);
    lua_setfield(state, -2, "__index");
    lua_pushcfunction(state, lua_error_code_node_newindex);
    lua_setfield(state, -2, "__newindex");
  }
  lua_pop(state, 1);
}

void lua_error_push_code_node(lua_State *state, const char *code,
                              size_t code_size) {
  lua_error_install_code_node_metatable(state);
  lua_newtable(state);
  lua_pushlstring(state, code, code_size);
  lua_setfield(state, -2, "code");
  luaL_getmetatable(state, LUA_ERROR_CODE_NODE_METATABLE);
  lua_setmetatable(state, -2);
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters): code bounds and offset.
void lua_error_code_tree_add(lua_State *state, int root, const char *code,
                             size_t code_size, size_t start) {
  int top = lua_gettop(state);
  int node;
  size_t segment_start = start;

  if (root < 0 && root > LUA_REGISTRYINDEX)
    root = top + root + 1;
  node = root;
  while (segment_start < code_size) {
    size_t segment_end = segment_start;

    while (segment_end < code_size &&
           lua_error_code_character_at(code, code_size, segment_end) != '.')
      segment_end++;
    lua_pushlstring(state, lua_error_code_at(code, code_size, segment_start),
                    segment_end - segment_start);
    lua_rawget(state, node);
    if (lua_isnil(state, -1)) {
      lua_pop(state, 1);
      lua_error_push_code_node(state, code, segment_end);
      lua_pushlstring(state, lua_error_code_at(code, code_size, segment_start),
                      segment_end - segment_start);
      lua_pushvalue(state, -2);
      lua_rawset(state, node);
    }
    node = lua_gettop(state);
    segment_start = segment_end + 1;
  }
  lua_settop(state, top);
}
// NOLINTEND(bugprone-easily-swappable-parameters)

static void lua_error_install_code_trees(lua_State *state) {
  int trees;

  lua_getfield(state, LUA_REGISTRYINDEX, LUA_ERROR_CODE_TREES_KEY);
  if (lua_istable(state, -1))
    return;
  lua_pop(state, 1);
  lua_newtable(state);
  trees = lua_gettop(state);
  for (int index = 0; index < LUA_ERROR_CODE_COUNT; index++) {
    const char *code = *(const char *const *)checked_storage_at_const(
        (const void *)LUA_ERROR_CODE_NAMES, LUA_ERROR_CODE_COUNT,
        sizeof(*LUA_ERROR_CODE_NAMES), (size_t)index);
    size_t code_size = strlen(code);
    size_t root_size = 0;

    while (root_size < code_size &&
           lua_error_code_character_at(code, code_size, root_size) != '.')
      root_size++;
    lua_pushlstring(state, code, root_size);
    lua_rawget(state, -2);
    if (lua_isnil(state, -1)) {
      lua_pop(state, 1);
      lua_error_push_code_node(state, code, root_size);
      lua_pushlstring(state, code, root_size);
      lua_pushvalue(state, -2);
      lua_rawset(state, trees);
    }
    lua_error_code_tree_add(state, -1, code, code_size,
                            root_size < code_size ? root_size + 1 : root_size);
    lua_pop(state, 1);
  }
  lua_pushvalue(state, -1);
  lua_setfield(state, LUA_REGISTRYINDEX, LUA_ERROR_CODE_TREES_KEY);
}

bool lua_error_push_code_tree(lua_State *state, const char *root) {
  lua_error_install_code_trees(state);
  lua_pushstring(state, root);
  lua_rawget(state, -2);
  lua_remove(state, -2);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  return true;
}

static int lua_error_tostring(lua_State *state) {
  const char *code;
  const char *message;

  lua_getfield(state, 1, "code");
  code = lua_tostring(state, -1);
  lua_getfield(state, 1, "message");
  message = lua_tostring(state, -1);
  lua_pushfstring(state, "%s: %s", code ? code : "lua.error",
                  message ? message : "unknown Lua error");
  return 1;
}

static int lua_error_is_method(lua_State *state) {
  const char *code = lua_error_check_code(state, 2);

  lua_pushboolean(state, lua_error_is(state, 1, code));
  return 1;
}

static int lua_error_root_method(lua_State *state) {
  constexpr int ROOT_LIMIT = 64;
  int depth = 0;

  lua_pushvalue(state, 1);
  while (lua_istable(state, -1) && depth < ROOT_LIMIT) {
    lua_getfield(state, -1, "cause");
    if (!lua_istable(state, -1)) {
      lua_pop(state, 1);
      break;
    }
    lua_remove(state, -2);
    depth++;
  }
  return 1;
}

void lua_error_install(lua_State *state) {
  if (luaL_newmetatable(state, LUA_ERROR_METATABLE)) {
    lua_pushcfunction(state, lua_error_tostring);
    lua_setfield(state, -2, "__tostring");
    lua_newtable(state);
    lua_pushcfunction(state, lua_error_is_method);
    lua_setfield(state, -2, "is");
    lua_pushcfunction(state, lua_error_root_method);
    lua_setfield(state, -2, "root");
    lua_setfield(state, -2, "__index");
  }
  lua_pop(state, 1);
}

void lua_error_push(lua_State *state, const char *code, const char *message) {
  lua_error_install(state);
  lua_createtable(state, 0, 2);
  lua_pushstring(state, code ? code : "lua.error");
  lua_setfield(state, -2, "code");
  lua_pushstring(state, message ? message : "unknown Lua error");
  lua_setfield(state, -2, "message");
  luaL_getmetatable(state, LUA_ERROR_METATABLE);
  lua_setmetatable(state, -2);
}

static int lua_error_vraise(lua_State *state, LuaErrorCode code,
                            const char *format, va_list arguments)
    __attribute__((format(printf, 3, 0)));

static int lua_error_vraise(lua_State *state, LuaErrorCode code,
                            const char *format, va_list arguments) {
  char message[2048];

  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(message, sizeof(message), format, arguments);
  lua_error_push(state, lua_error_code_name(code), message);
  return lua_error(state);
}

int lua_error_raise(lua_State *state, LuaErrorCode code, const char *format,
                    ...) {
  va_list arguments;
  int result;

  va_start(arguments, format);
  result = lua_error_vraise(state, code, format, arguments);
  va_end(arguments);
  return result;
}

int lua_error_arg(lua_State *state, int argument, LuaErrorCode code,
                  const char *format, ...) {
  va_list arguments;
  char detail[2048];
  char message[2304];
  char where[256];
  lua_Debug debug;
  const char *name = "?";
  bool is_method = false;

  va_start(arguments, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(detail, sizeof(detail), format, arguments);
  va_end(arguments);
  if (lua_getstack(state, 0, &debug) && lua_getinfo(state, "n", &debug)) {
    if (debug.name)
      name = debug.name;
    is_method = strcmp(debug.namewhat, "method") == 0;
  }
  if (is_method && argument > 0)
    argument--;
  luaL_where(state, 1);
  (void)snprintf(where, sizeof(where), "%s", lua_tostring(state, -1));
  lua_pop(state, 1);
  if (argument == 0)
    (void)snprintf(message, sizeof(message), "%scalling '%s' on bad self (%s)",
                   where, name, detail);
  else
    (void)snprintf(message, sizeof(message), "%sbad argument #%d to '%s' (%s)",
                   where, argument, name, detail);
  lua_error_push(state, lua_error_code_name(code), message);
  lua_newtable(state);
  lua_pushinteger(state, argument);
  lua_setfield(state, -2, "argument");
  lua_setfield(state, -2, "detail");
  return lua_error(state);
}

bool lua_error_field(lua_State *state, int index, const char *field, char *out,
                     size_t out_size) {
  bool found = false;

  if (index < 0 && index > LUA_REGISTRYINDEX)
    index = lua_gettop(state) + index + 1;
  if (lua_istable(state, index)) {
    lua_getfield(state, index, field);
    if (lua_isstring(state, -1)) {
      lua_error_copy(out, out_size, lua_tostring(state, -1));
      found = true;
    }
    lua_pop(state, 1);
  }
  return found;
}

const char *lua_error_check_code(lua_State *state, int index) {
  if (lua_istable(state, index)) {
    lua_getfield(state, index, "code");
    return luaL_checkstring(state, -1);
  }
  return luaL_checkstring(state, index);
}

bool lua_error_is(lua_State *state, int index, const char *code) {
  char actual[256];

  if (!lua_error_field(state, index, "code", actual, sizeof(actual)))
    return false;
  return lua_error_code_matches(actual, code);
}

void lua_error_describe(lua_State *state, int index, char *out,
                        size_t out_size) {
  char code[256];
  char message[2048];
  const char *value;

  if (!out || !out_size)
    return;
  if (lua_error_field(state, index, "code", code, sizeof(code)) &&
      lua_error_field(state, index, "message", message, sizeof(message))) {
    (void)snprintf(out, out_size, "%s: %s", code, message);
    return;
  }
  value = lua_tostring(state, index);
  if (value) {
    lua_error_copy(out, out_size, value);
    return;
  }
  lua_error_copy(out, out_size, luaL_typename(state, index));
}

// NOLINTEND(bugprone-easily-swappable-parameters)
