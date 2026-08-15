/* command_access.c - Access levels for Lua command entries. */

#include "mux/lua/command_access.h"

#include <lua.h>
#include <string.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

bool lua_command_access_read(lua_State *state, int entry,
                             LuaCommandAccess *access) {
  const char *value;
  bool valid = true;

  lua_getfield(state, entry, "access");
  if (lua_isnil(state, -1)) {
    *access = LUA_COMMAND_ACCESS_PUBLIC;
  } else if (lua_type(state, -1) != LUA_TSTRING) {
    valid = false;
  } else {
    value = lua_tostring(state, -1);
    if (!strcmp(value, "public"))
      *access = LUA_COMMAND_ACCESS_PUBLIC;
    else if (!strcmp(value, "wizard"))
      *access = LUA_COMMAND_ACCESS_WIZARD;
    else if (!strcmp(value, "god"))
      *access = LUA_COMMAND_ACCESS_GOD;
    else
      valid = false;
  }
  lua_pop(state, 1);
  return valid;
}

bool lua_command_access_allows(const LuaCommandAccessRequest *request) {
  GameDatabase *database = request->database;
  DbRef player = request->player;
  switch (request->access) {
  case LUA_COMMAND_ACCESS_PUBLIC:
    return true;
  case LUA_COMMAND_ACCESS_WIZARD:
    return (is_god(database, player) ||
            (player >= 0 && player < database->top &&
             is_wizard(database, player))) != 0;
  case LUA_COMMAND_ACCESS_GOD:
    return is_god(database, player);
  case LUA_COMMAND_ACCESS_COUNT:
    return false;
  }
  return false;
}

bool lua_command_entry_read(lua_State *state, int entry, GameDatabase *database,
                            DbRef player, const char **pattern) {
  LuaCommandAccess access;
  bool valid;

  if (!lua_istable(state, entry) ||
      !lua_command_access_read(state, entry, &access) ||
      !lua_command_access_allows(&(LuaCommandAccessRequest){
          .database = database, .player = player, .access = access}))
    return false;
  lua_getfield(state, entry, "pattern");
  *pattern = lua_tostring(state, -1);
  lua_pop(state, 1);
  lua_getfield(state, entry, "handler");
  valid = ((*pattern != nullptr && lua_isfunction(state, -1)) != 0);
  lua_pop(state, 1);
  return valid;
}
