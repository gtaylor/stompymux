/** @file
 * Structured Lua errors shared by native bindings and Lua authors.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>

#include <lua.h>

#include "mux/lua/lua_error_codes.h"

extern const char LUA_ERROR_METATABLE[];
extern const char LUA_TRACEBACK_KEY[];

/** Executes lua error install. @param[in,out] state State to inspect or update.
 */

void lua_error_install(lua_State *state);
/** Pushes lua error. @param[in,out] state State to inspect or update.
 * @param[in] code Code. @param[in] message Message. */

void lua_error_push(lua_State *state, const char *code, const char *message);
/** Executes lua error push code node. @param[in,out] state State to inspect or
 * update. @param[in] code Code. @param[in] code_size Size of code in bytes. */

void lua_error_push_code_node(lua_State *state, const char *code,
                              size_t code_size);
/** Adds lua error code tree. @param[in,out] state State to inspect or update.
 * @param[in] root Root. @param[in] code Code. @param[in] code_size Size of code
 * in bytes. @param[in] start Start. */

void lua_error_code_tree_add(lua_State *state, int root, const char *code,
                             size_t code_size, size_t start);
/** Executes lua error push code tree. @param[in,out] state State to inspect or
 * update. @param[in] root Root. */

bool lua_error_push_code_tree(lua_State *state, const char *root);
/** Executes lua error code name. @param[in] code Code. */

const char *lua_error_code_name(LuaErrorCode code);
/** Executes lua error raise. @param[in,out] state State to inspect or update.
 * @param[in] code Code. @param[in] format Format. */

int lua_error_raise(lua_State *state, LuaErrorCode code, const char *format,
                    ...) __attribute__((format(printf, 3, 4)));
/** Executes lua error arg. @param[in,out] state State to inspect or update.
 * @param[in] argument Command argument. @param[in] code Code. @param[in] format
 * Format. */

int lua_error_arg(lua_State *state, int argument, LuaErrorCode code,
                  const char *format, ...)
    __attribute__((format(printf, 4, 5)));
/** Executes lua error describe. @param[in,out] state State to inspect or
 * update. @param[in] index Zero-based index. @param[out] out Out. @param[in]
 * out_size Size of out in bytes. */

void lua_error_describe(lua_State *state, int index, char *out,
                        size_t out_size);
/** Executes lua error field. @param[in,out] state State to inspect or update.
 * @param[in] index Zero-based index. @param[in] field Field. @param[out] out
 * Out. @param[in] out_size Size of out in bytes. */

bool lua_error_field(lua_State *state, int index, const char *field, char *out,
                     size_t out_size);
/** Executes lua error check code. @param[in,out] state State to inspect or
 * update. @param[in] index Zero-based index. */

const char *lua_error_check_code(lua_State *state, int index);
/** Executes lua error is. @param[in] state State to inspect or update.
 * @param[in] index Zero-based index. @param[in] code Code. */

bool lua_error_is(lua_State *state, int index, const char *code);
