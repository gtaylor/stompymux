/* mux_error_bindings.c - mux.error API. */

#include <lauxlib.h>
#include <limits.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/support/checked_storage.h"

static char lua_mux_error_code_character_at(const char *code, size_t code_size,
                                            size_t index) {
  return *(const char *)checked_storage_at_const(code, code_size, sizeof(char),
                                                 index);
}

static bool lua_mux_error_code_is_valid(const char *code, size_t code_size) {
  bool segment_start = true;

  if (!code_size)
    return false;
  for (size_t index = 0; index < code_size; index++) {
    char character = lua_mux_error_code_character_at(code, code_size, index);

    if (character == '.') {
      if (segment_start)
        return false;
      segment_start = true;
    } else if (segment_start) {
      if (character < 'a' || character > 'z')
        return false;
      segment_start = false;
    } else if (!((character >= 'a' && character <= 'z') ||
                 (character >= '0' && character <= '9') || character == '_')) {
      return false;
    }
  }
  return (bool)!segment_start;
}

static bool lua_mux_error_prefix_is_reserved(const char *prefix,
                                             size_t prefix_size) {
  size_t segment_size = 0;

  while (segment_size < prefix_size &&
         lua_mux_error_code_character_at(prefix, prefix_size, segment_size) !=
             '.')
    segment_size++;
  if (segment_size == 3 && !memcmp(prefix, "mux", segment_size))
    return true;
  if (segment_size == 5 && !memcmp(prefix, "btech", segment_size))
    return true;
  if (segment_size == 7 && !memcmp(prefix, "testing", segment_size))
    return true;
  return false;
}

static void lua_mux_error_normalize(lua_State *state, int index) {
  char message[2048];

  if (index < 0 && index > LUA_REGISTRYINDEX)
    index = lua_gettop(state) + index + 1;
  if (lua_istable(state, index)) {
    char code[256];

    if (lua_error_field(state, index, "code", code, sizeof(code))) {
      lua_pushvalue(state, index);
      return;
    }
  }
  lua_error_describe(state, index, message, sizeof(message));
  lua_error_push(state, lua_error_code_name(LUA_ERROR_CODE_RUNTIME), message);
}

/**
 * Creates a structured error value without raising it.
 *
 * @par Lua name `mux.error.new`
 * @par Lua signature `mux.error.new({ code = "area.missing", message = "area
 * was not found", detail = {} })`
 * @par Lua parameters - `fields` (`table`) An error definition with the
 * required code and message fields, and optional detail and cause fields.
 * - `fields.code` (`string|ErrorCode`) A dotted code string or a node from
 * mux.error.codes or mux.error.namespace.
 * - `fields.message` (`string`) A readable description of the error.
 * - `fields.detail` (`any`) Optional. Optional structured context for the
 * failure.
 * - `fields.cause` (`any`) Optional. Optional earlier error or other Lua value.
 * @par Lua returns - `error` (`Error`): The new structured error value.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for an invalid error code;
 * ordinary Lua type errors for invalid fields.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_new(lua_State *state) {
  const char *code;
  const char *message;

  luaL_checktype(state, 1, LUA_TTABLE);
  lua_getfield(state, 1, "code");
  code = lua_error_check_code(state, -1);
  lua_getfield(state, 1, "message");
  message = luaL_checkstring(state, -1);
  lua_error_push(state, code, message);
  lua_getfield(state, 1, "detail");
  if (!lua_isnil(state, -1))
    lua_setfield(state, -2, "detail");
  else
    lua_pop(state, 1);
  lua_getfield(state, 1, "cause");
  if (!lua_isnil(state, -1))
    lua_setfield(state, -2, "cause");
  else
    lua_pop(state, 1);
  return 1;
}

/**
 * Raises a structured error.
 *
 * @par Lua name `mux.error.raise`
 * @par Lua signature `mux.error.raise(code, message [, detail])`
 * @par Lua parameters - `code` (`string|ErrorCode`) A dotted code string or a
 * node from mux.error.codes or mux.error.namespace.
 * - `message` (`string`) A readable description of the failure.
 * - `detail` (`any`) Optional. Optional structured context stored on the raised
 * Error.
 * @par Lua returns - No values.
 * @par Lua errors - Raises the requested stable code;
 * `LUA_ERROR_CODE_ARG_INVALID` if that code is invalid.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_raise(lua_State *state) {
  const char *code = lua_error_check_code(state, 1);
  const char *message = luaL_checkstring(state, 2);

  lua_error_push(state, code, message);
  if (!lua_isnoneornil(state, 3)) {
    lua_pushvalue(state, 3);
    lua_setfield(state, -2, "detail");
  }
  return lua_error(state);
}

/**
 * Tests whether an error-like value has a matching code.
 *
 * @par Lua name `mux.error.is`
 * @par Lua signature `mux.error.is(value, code)`
 * @par Lua parameters - `value` (`any`) A value to inspect. It matches only
 * when it is a table with a string code field.
 * - `code` (`string|ErrorCode`) A dotted code string or a node from
 * mux.error.codes or mux.error.namespace.
 * @par Lua returns - `matches` (`boolean`): true when value.code equals code or
 * begins with code followed by a dotted segment; otherwise false.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` when the requested comparison
 * code is invalid.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_is(lua_State *state) {
  const char *code = lua_error_check_code(state, 2);

  lua_pushboolean(state, lua_error_is(state, 1, code));
  return 1;
}

/**
 * Returns a successful value or raises the supplied failure.
 *
 * @par Lua name `mux.error.check`
 * @par Lua signature `mux.error.check(value, err)`
 * @par Lua parameters - `value` (`any`) The result to test. Any Lua truthy
 * value is returned unchanged.
 * - `err` (`any`) The Lua value to raise when value is false or nil; normally
 * an Error.
 * @par Lua returns - `value` (`any`): The original truthy value.
 * @par Lua errors - Raises `err` unchanged when `value` is false or nil; it
 * does not invent a stable code.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_check(lua_State *state) {
  if (!lua_toboolean(state, 1)) {
    lua_pushvalue(state, 2);
    return lua_error(state);
  }
  lua_settop(state, 1);
  return 1;
}

/**
 * Creates a structured error that preserves an earlier failure as its cause.
 *
 * @par Lua name `mux.error.wrap`
 * @par Lua signature `mux.error.wrap(err, code, message)`
 * @par Lua parameters - `err` (`any`) The earlier failure. A table with a
 * string code field is retained; another Lua value is normalized to a
 * mux.runtime error before becoming the cause.
 * - `code` (`string|ErrorCode`) A dotted code string or a node from
 * mux.error.codes or mux.error.namespace.
 * - `message` (`string`) A readable description of the additional context.
 * @par Lua returns - `error` (`Error`): A new error with error.cause set to the
 * retained or normalized err.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for an invalid wrapper code;
 * non-error causes are normalized to `LUA_ERROR_CODE_RUNTIME`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_wrap(lua_State *state) {
  const char *code = lua_error_check_code(state, 2);
  const char *message = luaL_checkstring(state, 3);

  lua_error_push(state, code, message);
  lua_mux_error_normalize(state, 1);
  lua_setfield(state, -2, "cause");
  return 1;
}

/**
 * Builds a checked code-symbol tree for an author-defined error namespace.
 *
 * @par Lua name `local codes = mux.error.namespace`
 * @par Lua signature `local codes = mux.error.namespace("cargo", { "full",
 * "bay.locked" })`
 * @par Lua parameters - `prefix` (`string`) The first code segment, or a dotted
 * series of lower-case segments. The first segment must not be mux, btech, or
 * testing.
 * - `names` (`string[]`) An array of leaf names relative to prefix. Every name
 * uses one or more lower-case dotted segments; later segments may also contain
 * digits or _.
 * @par Lua returns - `codes` (`table`): A code-symbol tree whose leaf and
 * intermediate nodes can be passed wherever mux.error accepts a code.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for invalid/reserved prefixes,
 * invalid names, or too many names.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_namespace(lua_State *state) {
  const char *prefix;
  size_t prefix_size;
  size_t name_count;

  prefix = luaL_checklstring(state, 1, &prefix_size);
  luaL_checktype(state, 2, LUA_TTABLE);
  if (!lua_mux_error_code_is_valid(prefix, prefix_size))
    return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                           "namespace prefix must contain dotted lower-case "
                           "segments");
  if (lua_mux_error_prefix_is_reserved(prefix, prefix_size))
    return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                           "namespace prefix '%s' is reserved", prefix);
  name_count = lua_objlen(state, 2);
  if (name_count > INT_MAX)
    return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                           "namespace contains too many names");
  lua_error_push_code_node(state, prefix, prefix_size);
  for (int index = 1; index <= (int)name_count; index++) {
    const char *name;
    const char *code;
    size_t name_size;
    size_t code_size;

    lua_rawgeti(state, 2, index);
    name = luaL_checklstring(state, -1, &name_size);
    if (!lua_mux_error_code_is_valid(name, name_size))
      return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                             "namespace names must contain dotted lower-case "
                             "segments");
    lua_pushlstring(state, prefix, prefix_size);
    lua_pushliteral(state, ".");
    lua_pushlstring(state, name, name_size);
    lua_concat(state, 3);
    code = lua_tolstring(state, -1, &code_size);
    lua_error_code_tree_add(state, 3, code, code_size, prefix_size + 1);
    lua_pop(state, 2);
  }
  return 1;
}

/**
 * Returns the checked native code tree for a namespace root.
 *
 * @par Lua name `local btech_codes = mux.error.code_tree`
 * @par Lua signature `local btech_codes = mux.error.code_tree("btech")`
 * @par Lua parameters - `root` (`string`) A native namespace root: mux, btech,
 * or testing.
 * @par Lua returns - `codes` (`table`): The cached checked symbol tree for the
 * root. Repeated calls for the same root return the same table object.  Unknown
 * roots raise mux.arg.invalid.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for an unknown native
 * namespace root.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_code_tree(lua_State *state) {
  const char *root = luaL_checkstring(state, 1);

  if (!lua_error_push_code_tree(state, root))
    return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                           "unknown Lua error code root '%s'", root);
  return 1;
}

/**
 * Calls a function and returns either its results or a normalized error with a
 * traceback.
 *
 * @par Lua name `ok, result_or_error = mux.error.pcall`
 * @par Lua signature `ok, result_or_error = mux.error.pcall(fn, ...)`
 * @par Lua parameters - `fn` (`function`) The function to call.
 * - `...` (`any...`) Arguments passed to fn.
 * @par Lua returns - `...` (`true|any...`): On success, true followed by every
 * value returned by fn.
 * - `error` (`false|table`): On failure, false and the raised error. A raised
 * table with a string code field is retained; another Lua value is converted to
 * a mux.runtime error. The returned error receives a traceback field.
 * @par Lua errors - Does not raise a native error; failures are returned and
 * non-error failures are normalized to `LUA_ERROR_CODE_RUNTIME`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_error_pcall(lua_State *state) {
  int arguments = lua_gettop(state) - 1;
  int status;
  int results;

  luaL_checktype(state, 1, LUA_TFUNCTION);
  status = lua_pcall(state, arguments, LUA_MULTRET, 0);
  if (!status) {
    results = lua_gettop(state);

    lua_pushboolean(state, 1);
    lua_insert(state, 1);
    return results + 1;
  }
  lua_getfield(state, LUA_REGISTRYINDEX, LUA_TRACEBACK_KEY);
  if (lua_isfunction(state, -1)) {
    lua_pushvalue(state, -2);
    lua_pushinteger(state, 2);
    if (lua_pcall(state, 2, 1, 0) == 0) {
      lua_mux_error_normalize(state, -2);
      lua_remove(state, -3);
      lua_pushvalue(state, -2);
      lua_setfield(state, -2, "traceback");
      lua_remove(state, -2);
      lua_pushboolean(state, 0);
      lua_insert(state, -2);
      return 2;
    }
  }
  lua_pop(state, 1);
  lua_mux_error_normalize(state, -1);
  lua_remove(state, -2);
  lua_pushliteral(state, "Lua traceback unavailable");
  lua_setfield(state, -2, "traceback");
  lua_pushboolean(state, 0);
  lua_insert(state, -2);
  return 2;
}

void lua_mux_install_error_bindings(lua_State *state,
                                    LuaMuxPackage *package [[maybe_unused]]) {
  lua_error_install(state);
  lua_newtable(state);
  lua_pushcfunction(state, lua_mux_error_new);
  lua_setfield(state, -2, "new");
  lua_pushcfunction(state, lua_mux_error_raise);
  lua_setfield(state, -2, "raise");
  lua_pushcfunction(state, lua_mux_error_is);
  lua_setfield(state, -2, "is");
  lua_pushcfunction(state, lua_mux_error_check);
  lua_setfield(state, -2, "check");
  lua_pushcfunction(state, lua_mux_error_wrap);
  lua_setfield(state, -2, "wrap");
  lua_pushcfunction(state, lua_mux_error_pcall);
  lua_setfield(state, -2, "pcall");
  lua_pushcfunction(state, lua_mux_error_namespace);
  lua_setfield(state, -2, "namespace");
  lua_pushcfunction(state, lua_mux_error_code_tree);
  lua_setfield(state, -2, "code_tree");
  if (!lua_error_push_code_tree(state, "mux")) {
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "native mux error code tree is unavailable");
    return;
  }
  lua_setfield(state, -2, "codes");
  lua_setfield(state, -2, "error");
}
