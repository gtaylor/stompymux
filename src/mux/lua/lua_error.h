/* Structured Lua errors shared by native bindings and Lua authors. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include <lua.h>

#include "mux/lua/lua_error_codes.h"

extern const char LUA_ERROR_METATABLE[];
extern const char LUA_TRACEBACK_KEY[];

void lua_error_install(lua_State *state);
void lua_error_push(lua_State *state, const char *code, const char *message);
void lua_error_push_code_node(lua_State *state, const char *code,
                              size_t code_size);
void lua_error_code_tree_add(lua_State *state, int root, const char *code,
                             size_t code_size, size_t start);
bool lua_error_push_code_tree(lua_State *state, const char *root);
const char *lua_error_code_name(LuaErrorCode code);
int lua_error_raise(lua_State *state, LuaErrorCode code, const char *format,
                    ...) __attribute__((format(printf, 3, 4)));
int lua_error_arg(lua_State *state, int argument, LuaErrorCode code,
                  const char *format, ...)
    __attribute__((format(printf, 4, 5)));
void lua_error_describe(lua_State *state, int index, char *out,
                        size_t out_size);
bool lua_error_field(lua_State *state, int index, const char *field, char *out,
                     size_t out_size);
const char *lua_error_check_code(lua_State *state, int index);
bool lua_error_is(lua_State *state, int index, const char *code);
