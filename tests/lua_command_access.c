/* lua_command_access.c -- Lua command access parsing and role checks */

#include "mux/lua/command_access.h"
#include "mux/objects/flags.h"

#include <lauxlib.h>
#include <lua.h>

#include <string.h>

static bool read_access(lua_State *state, const char *value,
                        LuaCommandAccess expected) {
  LuaCommandAccess access = LUA_COMMAND_ACCESS_COUNT;
  bool valid;

  lua_newtable(state);
  if (value != nullptr) {
    lua_pushstring(state, value);
    lua_setfield(state, -2, "access");
  }
  valid = lua_command_access_read(state, lua_gettop(state), &access);
  lua_pop(state, 1);
  return valid && access == expected;
}

static int command_handler(lua_State *state) { return 0; }

static bool read_entry(lua_State *state, GameDatabase *database, DbRef player,
                       const char *access, bool include_pattern,
                       bool include_handler) {
  const char *pattern = nullptr;
  bool readable;

  lua_newtable(state);
  if (access != nullptr) {
    lua_pushstring(state, access);
    lua_setfield(state, -2, "access");
  }
  if (include_pattern) {
    lua_pushstring(state, "^test$");
    lua_setfield(state, -2, "pattern");
  }
  if (include_handler) {
    lua_pushcfunction(state, command_handler);
    lua_setfield(state, -2, "handler");
  }
  readable = lua_command_entry_read(state, lua_gettop(state), database, player,
                                    &pattern);
  lua_pop(state, 1);
  return readable && pattern != nullptr && !strcmp(pattern, "^test$");
}

int main(void) {
  lua_State *state = luaL_newstate();
  GameObject objects[5] = {0};
  GameDatabase database = {.object_storage = objects, .top = 4, .size = 4};
  LuaCommandAccess access;

  if (state == nullptr)
    return 2;
  game_database_object(&database, GOD)->type = OBJECT_TYPE_PLAYER;
  game_database_object(&database, GOD)->has_wizard_flag = true;
  game_database_object(&database, 2)->type = OBJECT_TYPE_PLAYER;
  game_database_object(&database, 2)->has_wizard_flag = true;
  game_database_object(&database, 3)->type = OBJECT_TYPE_PLAYER;

  if (!read_access(state, nullptr, LUA_COMMAND_ACCESS_PUBLIC) ||
      !read_access(state, "public", LUA_COMMAND_ACCESS_PUBLIC) ||
      !read_access(state, "wizard", LUA_COMMAND_ACCESS_WIZARD) ||
      !read_access(state, "god", LUA_COMMAND_ACCESS_GOD) ||
      read_access(state, "Wizard", LUA_COMMAND_ACCESS_WIZARD) ||
      read_access(state, "everyone", LUA_COMMAND_ACCESS_PUBLIC)) {
    lua_close(state);
    return 1;
  }
  lua_newtable(state);
  lua_pushinteger(state, 1);
  lua_setfield(state, -2, "access");
  if (lua_command_access_read(state, lua_gettop(state), &access)) {
    lua_close(state);
    return 1;
  }
  lua_pop(state, 1);

  if (!lua_command_access_allows(
          &(LuaCommandAccessRequest){.database = &database,
                                     .player = 3,
                                     .access = LUA_COMMAND_ACCESS_PUBLIC}) ||
      lua_command_access_allows(
          &(LuaCommandAccessRequest){.database = &database,
                                     .player = 3,
                                     .access = LUA_COMMAND_ACCESS_WIZARD}) ||
      lua_command_access_allows(
          &(LuaCommandAccessRequest){.database = &database,
                                     .player = 3,
                                     .access = LUA_COMMAND_ACCESS_GOD}) ||
      !lua_command_access_allows(
          &(LuaCommandAccessRequest){.database = &database,
                                     .player = 2,
                                     .access = LUA_COMMAND_ACCESS_WIZARD}) ||
      lua_command_access_allows(
          &(LuaCommandAccessRequest){.database = &database,
                                     .player = 2,
                                     .access = LUA_COMMAND_ACCESS_GOD}) ||
      !lua_command_access_allows(
          &(LuaCommandAccessRequest){.database = &database,
                                     .player = GOD,
                                     .access = LUA_COMMAND_ACCESS_WIZARD}) ||
      !lua_command_access_allows(
          &(LuaCommandAccessRequest){.database = &database,
                                     .player = GOD,
                                     .access = LUA_COMMAND_ACCESS_GOD})) {
    lua_close(state);
    return 1;
  }

  if (!read_entry(state, &database, 3, nullptr, true, true) ||
      read_entry(state, &database, 3, "wizard", true, true) ||
      !read_entry(state, &database, 2, "wizard", true, true) ||
      read_entry(state, &database, 2, "god", true, true) ||
      !read_entry(state, &database, GOD, "god", true, true) ||
      read_entry(state, &database, GOD, "god", false, true) ||
      read_entry(state, &database, GOD, "god", true, false) ||
      read_entry(state, &database, GOD, "invalid", true, true)) {
    lua_close(state);
    return 1;
  }

  lua_close(state);
  return 0;
}
