/** @file
 * Access levels for Lua command entries.
 */
#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"

typedef struct lua_State LuaState;

typedef enum LuaCommandAccess : int {
  LUA_COMMAND_ACCESS_PUBLIC,
  LUA_COMMAND_ACCESS_WIZARD,
  LUA_COMMAND_ACCESS_GOD,
  LUA_COMMAND_ACCESS_COUNT,
} LuaCommandAccess;

/** Executes lua command access read. @param[in,out] state State to inspect or
 * update. @param[in] entry Entry. @param[in,out] access Access. */

bool lua_command_access_read(LuaState *state, int entry,
                             LuaCommandAccess *access);

/** Installs the immutable `mux.world.access` namespace into the table at the
 * top of the Lua stack. @param[in,out] state Lua state. */

void lua_command_access_install_namespace(LuaState *state);

typedef struct LuaCommandAccessRequest {
  GameDatabase *database;
  DbRef player;
  LuaCommandAccess access;
} LuaCommandAccessRequest;

/** Executes lua command access allows. @param[in] request Request. */

bool lua_command_access_allows(const LuaCommandAccessRequest *request);
/** Executes lua command entry read. @param[in,out] state State to inspect or
 * update. @param[in] entry Entry. @param[in,out] database Game database.
 * @param[in] player Player object. @param[in,out] pattern Pattern. */

bool lua_command_entry_read(LuaState *state, int entry, GameDatabase *database,
                            DbRef player, const char **pattern);
