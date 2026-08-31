/** @file
 * Canonical Lua object-lock catalog.
 */
#pragma once

#include <stddef.h>

#include "mux/lua/lua_runtime.h"

typedef struct LuaLockDefinition LuaLockDefinition;
struct LuaLockDefinition {
  LuaLockType type;
  const char *key;
  const char *constant;
};

/** Returns the number of supported Lua locks. */
size_t lua_lock_definition_count(void);

/** Returns a lock definition by catalog index.
 * @param[in] index Zero-based catalog index. */
const LuaLockDefinition *lua_lock_definition_at(size_t index);

/** Finds a lock definition by object-module key.
 * @param[in] key Exact lowercase object-module key. */
const LuaLockDefinition *lua_lock_definition_find_key(const char *key);

/** Finds a lock definition by Lua constant name.
 * @param[in] constant Exact uppercase constant name. */
const LuaLockDefinition *
lua_lock_definition_find_constant(const char *constant);
