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

/**
 * @par LuaLS definition mux alias error.root
 * @code{.lua}
 * ---@alias NativeErrorRoot "mux"|"btech"|"testing" Root of a checked native error-code tree.
 * @endcode
 *
 * @par LuaLS definition mux type error.code
 * @code{.lua}
 * ---A checked error-code symbol. Calling `tostring` returns its dotted `code`.
 * ---@class ErrorCode
 * ---@field code string Fully qualified error code represented by this node.
 * @endcode
 *
 * @par LuaLS definition mux type error.value
 * @code{.lua}
 * ---A structured Lua failure raised by native and script APIs. Calling `tostring`
 * ---renders its stable code followed by its human-readable message.
 * ---@class Error
 * ---@field code string Stable dotted error code.
 * ---@field message string Human-readable failure description.
 * ---@field detail? any Optional structured context.
 * ---@field cause? any Earlier failure preserved by wrapping.
 * ---@field traceback? string Traceback added by [`mux.error.pcall`](lua://mux.error.pcall).
 * local Error = {}
 * @endcode
 *
 * @par LuaLS definition mux type error.caught
 * @code{.lua}
 * ---The minimum shape returned when [`mux.error.pcall`](lua://mux.error.pcall)
 * ---catches a table carrying a string error code. Such a table is not guaranteed
 * ---to use the native [`Error`](lua://Error) metatable.
 * ---@class CaughtError
 * ---@field code string Stable dotted error code.
 * ---@field traceback string Traceback captured by `mux.error.pcall`.
 * ---@field message? string Optional human-readable failure description.
 * ---@field detail? any Optional structured context.
 * ---@field cause? any Optional earlier failure.
 * @endcode
 *
 * @par LuaLS definition mux type error.fields
 * @code{.lua}
 * ---@class ErrorFields
 * ---@field code string|ErrorCode Stable dotted code or checked code node.
 * ---@field message string Human-readable failure description.
 * ---@field detail? any Optional structured context.
 * ---@field cause? any Optional earlier failure.
 * @endcode
 *
 * @par LuaLS definition mux catalog mux.error.codes
 * @code{.lua}
 * ---Checked `mux.arg.invalid` error-code node.
 * ---@class MuxArgInvalidErrorCode: ErrorCode
 * ---@field code "mux.arg.invalid"
 * ---Checked `mux.unavailable.checking` error-code node.
 * ---@class MuxCheckingUnavailableErrorCode: ErrorCode
 * ---@field code "mux.unavailable.checking"
 * ---Checked `mux.runtime` error-code node.
 * ---@class MuxRuntimeErrorCode: ErrorCode
 * ---@field code "mux.runtime"
 * ---Checked `mux.state.invalid` error-code node.
 * ---@class MuxStateInvalidErrorCode: ErrorCode
 * ---@field code "mux.state.invalid"
 * ---Checked `mux.state.value_too_large` error-code node.
 * ---@class MuxStateValueTooLargeErrorCode: ErrorCode
 * ---@field code "mux.state.value_too_large"
 * ---Checked `mux.state.unavailable` error-code node.
 * ---@class MuxStateUnavailableErrorCode: ErrorCode
 * ---@field code "mux.state.unavailable"
 * ---Checked `mux.object.invalid` error-code node.
 * ---@class MuxObjectInvalidErrorCode: ErrorCode
 * ---@field code "mux.object.invalid"
 * ---Checked `mux.object.unavailable` error-code node.
 * ---@class MuxObjectUnavailableErrorCode: ErrorCode
 * ---@field code "mux.object.unavailable"
 * ---Checked `mux.attribute.invalid` error-code node.
 * ---@class MuxAttributeInvalidErrorCode: ErrorCode
 * ---@field code "mux.attribute.invalid"
 * ---Checked `mux.flag.invalid` error-code node.
 * ---@class MuxFlagInvalidErrorCode: ErrorCode
 * ---@field code "mux.flag.invalid"
 * ---Checked `mux.power.invalid` error-code node.
 * ---@class MuxPowerInvalidErrorCode: ErrorCode
 * ---@field code "mux.power.invalid"
 * ---Checked `mux.access.invalid` error-code node.
 * ---@class MuxAccessInvalidErrorCode: ErrorCode
 * ---@field code "mux.access.invalid"
 * ---Checked `mux.connection.invalid` error-code node.
 * ---@class MuxConnectionInvalidErrorCode: ErrorCode
 * ---@field code "mux.connection.invalid"
 * ---Checked `mux.connection.unavailable` error-code node.
 * ---@class MuxConnectionUnavailableErrorCode: ErrorCode
 * ---@field code "mux.connection.unavailable"
 * ---Checked `mux.channel.invalid` error-code node.
 * ---@class MuxChannelInvalidErrorCode: ErrorCode
 * ---@field code "mux.channel.invalid"
 * ---Checked `mux.channel_flag.invalid` error-code node.
 * ---@class MuxChannelFlagInvalidErrorCode: ErrorCode
 * ---@field code "mux.channel_flag.invalid"
 * ---Checked `mux.text.invalid` error-code node.
 * ---@class MuxTextInvalidErrorCode: ErrorCode
 * ---@field code "mux.text.invalid"
 * ---Checked `mux.module.invalid` error-code node.
 * ---@class MuxModuleInvalidErrorCode: ErrorCode
 * ---@field code "mux.module.invalid"
 * ---Checked `mux.module.unavailable` error-code node.
 * ---@class MuxModuleUnavailableErrorCode: ErrorCode
 * ---@field code "mux.module.unavailable"
 * ---Checked `mux.config.not_found` error-code node.
 * ---@class MuxConfigNotFoundErrorCode: ErrorCode
 * ---@field code "mux.config.not_found"
 * ---Checked `mux.config.unsupported` error-code node.
 * ---@class MuxConfigUnsupportedErrorCode: ErrorCode
 * ---@field code "mux.config.unsupported"
 * ---Checked `mux.internal` error-code node.
 * ---@class MuxInternalErrorCode: ErrorCode
 * ---@field code "mux.internal"
 *
 * ---@class MuxArgErrorCodes: ErrorCode
 * ---@field invalid MuxArgInvalidErrorCode `mux.arg.invalid`.
 * ---@class MuxUnavailableErrorCodes: ErrorCode
 * ---@field checking MuxCheckingUnavailableErrorCode `mux.unavailable.checking`.
 * ---@class MuxStateErrorCodes: ErrorCode
 * ---@field invalid MuxStateInvalidErrorCode `mux.state.invalid`.
 * ---@field value_too_large MuxStateValueTooLargeErrorCode `mux.state.value_too_large`.
 * ---@field unavailable MuxStateUnavailableErrorCode `mux.state.unavailable`.
 * ---@class MuxObjectErrorCodes: ErrorCode
 * ---@field invalid MuxObjectInvalidErrorCode `mux.object.invalid`.
 * ---@field unavailable MuxObjectUnavailableErrorCode `mux.object.unavailable`.
 * ---@class MuxAttributeErrorCodes: ErrorCode
 * ---@field invalid MuxAttributeInvalidErrorCode `mux.attribute.invalid`.
 * ---@class MuxFlagErrorCodes: ErrorCode
 * ---@field invalid MuxFlagInvalidErrorCode `mux.flag.invalid`.
 * ---@class MuxPowerErrorCodes: ErrorCode
 * ---@field invalid MuxPowerInvalidErrorCode `mux.power.invalid`.
 * ---@class MuxAccessErrorCodes: ErrorCode
 * ---@field invalid MuxAccessInvalidErrorCode `mux.access.invalid`.
 * ---@class MuxConnectionErrorCodes: ErrorCode
 * ---@field invalid MuxConnectionInvalidErrorCode `mux.connection.invalid`.
 * ---@field unavailable MuxConnectionUnavailableErrorCode `mux.connection.unavailable`.
 * ---@class MuxChannelErrorCodes: ErrorCode
 * ---@field invalid MuxChannelInvalidErrorCode `mux.channel.invalid`.
 * ---@class MuxChannelFlagErrorCodes: ErrorCode
 * ---@field invalid MuxChannelFlagInvalidErrorCode `mux.channel_flag.invalid`.
 * ---@class MuxTextErrorCodes: ErrorCode
 * ---@field invalid MuxTextInvalidErrorCode `mux.text.invalid`.
 * ---@class MuxModuleErrorCodes: ErrorCode
 * ---@field invalid MuxModuleInvalidErrorCode `mux.module.invalid`.
 * ---@field unavailable MuxModuleUnavailableErrorCode `mux.module.unavailable`.
 * ---@class MuxConfigErrorCodes: ErrorCode
 * ---@field not_found MuxConfigNotFoundErrorCode `mux.config.not_found`.
 * ---@field unsupported MuxConfigUnsupportedErrorCode `mux.config.unsupported`.
 * ---@class MuxErrorCodes: ErrorCode
 * ---@field arg MuxArgErrorCodes Invalid-argument code branch.
 * ---@field unavailable MuxUnavailableErrorCodes Runtime-availability code branch.
 * ---@field runtime MuxRuntimeErrorCode `mux.runtime`.
 * ---@field state MuxStateErrorCodes Persistent-state code branch.
 * ---@field object MuxObjectErrorCodes Database-object code branch.
 * ---@field attribute MuxAttributeErrorCodes Native-attribute code branch.
 * ---@field flag MuxFlagErrorCodes Object-flag code branch.
 * ---@field power MuxPowerErrorCodes Object-power code branch.
 * ---@field access MuxAccessErrorCodes Command-access code branch.
 * ---@field connection MuxConnectionErrorCodes Connection code branch.
 * ---@field channel MuxChannelErrorCodes Communication-channel code branch.
 * ---@field channel_flag MuxChannelFlagErrorCodes Channel-flag code branch.
 * ---@field text MuxTextErrorCodes Styled-text code branch.
 * ---@field module MuxModuleErrorCodes Lua-module code branch.
 * ---@field config MuxConfigErrorCodes Configuration code branch.
 * ---@field internal MuxInternalErrorCode `mux.internal`.
 * @endcode
 *
 * @par LuaLS definition mux type error.tree
 * @code{.lua}
 * ---@class ErrorCodeTree: ErrorCode
 * ---@field [string] ErrorCodeTree Checked child code segment.
 * @endcode
 *
 * @par LuaLS definition mux catalog mux.testing.codes
 * @code{.lua}
 * ---Checked `testing.assertion` error-code node used by the Lua test harness.
 * ---@class TestingAssertionErrorCode: ErrorCode
 * ---@field code "testing.assertion"
 * ---Checked `testing.runtime` error-code node used by the Lua test harness.
 * ---@class TestingRuntimeErrorCode: ErrorCode
 * ---@field code "testing.runtime"
 * ---Checked native code tree used by the Lua test harness.
 * ---@class TestingErrorCodes: ErrorCode
 * ---@field assertion TestingAssertionErrorCode `testing.assertion`.
 * ---@field runtime TestingRuntimeErrorCode `testing.runtime`.
 * @endcode
 *
 * @par LuaLS definition btech catalog btech.error.codes
 * @code{.lua}
 * ---Checked `btech.unavailable` error-code node.
 * ---@class BtechUnavailableErrorCode: ErrorCode
 * ---@field code "btech.unavailable"
 *
 * ---Checked `btech.failed` error-code node.
 * ---@class BtechFailedErrorCode: ErrorCode
 * ---@field code "btech.failed"
 *
 * ---Checked native BattleTech error-code tree.
 * ---@class BtechErrorCodes: ErrorCode
 * ---@field unavailable BtechUnavailableErrorCode `btech.unavailable`, raised during `@lua/check`.
 * ---@field failed BtechFailedErrorCode `btech.failed`, raised when a mapped legacy handler reports an error.
 * @endcode
 *
 * @par LuaLS definition btech type error.package
 * @code{.lua}
 * ---@class BtechErrorPackage
 * ---@field codes BtechErrorCodes Checked native BattleTech code tree.
 * @endcode
 */
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

/**
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the ErrorCode class declaration.
 */
static int lua_error_code_node_tostring(lua_State *state) {
  lua_getfield(state, 1, "code");
  return 1;
}

/**
 * @par LuaLS ignore mux __index -- Dynamic child lookup is represented by the ErrorCodeTree table declaration.
 */
static int lua_error_code_node_index(lua_State *state) {
  const char *key = lua_tostring(state, 2);

  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "unknown Lua error code segment '%s'",
                         key ? key : "<non-string>");
}

/**
 * @par LuaLS ignore mux __newindex -- Immutability is represented by the ErrorCode and ErrorCodeTree declarations.
 */
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

/**
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the Error class declaration.
 */
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

/**
 * Tests this error's code using dotted-prefix matching.
 *
 * @par LuaLS definition mux callable Error:is
 * @code{.lua}
 * ---Tests this error's code using dotted-prefix matching.
 * ---@param code string|ErrorCode
 * ---@return boolean matches
 * function Error:is(code) end
 * @endcode
 */
static int lua_error_is_method(lua_State *state) {
  const char *code = lua_error_check_code(state, 2);

  lua_pushboolean(state, lua_error_is(state, 1, code));
  return 1;
}

/**
 * Returns the deepest table-valued cause, or this error when it has none.
 *
 * @par LuaLS definition mux callable Error:root
 * @code{.lua}
 * ---Returns the deepest table-valued cause, or this error when it has none.
 * ---@return any root
 * function Error:root() end
 * @endcode
 */
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
