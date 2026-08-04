/* command_access.h - Access levels for Lua command entries. */

#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct lua_State lua_State;

typedef enum LuaCommandAccess {
  LUA_COMMAND_ACCESS_PUBLIC,
  LUA_COMMAND_ACCESS_WIZARD,
  LUA_COMMAND_ACCESS_GOD,
  LUA_COMMAND_ACCESS_COUNT,
} LuaCommandAccess;

bool lua_command_access_read(lua_State *state, int entry,
                             LuaCommandAccess *access);
bool lua_command_access_allows(GameDatabase *database, DbRef player,
                               LuaCommandAccess access);
bool lua_command_entry_read(lua_State *state, int entry, GameDatabase *database,
                            DbRef player, const char **pattern);
