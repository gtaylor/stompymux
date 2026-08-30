/* lua_command_access.c -- Lua command access parsing and role checks */

#include "mux/lua/command_access.h"
#include "mux/lua/lua_error.h"
#include "mux/objects/flags.h"

#include <lauxlib.h>
#include <lua.h>

#include <string.h>

static void push_access(lua_State *state, const char *name) {
  lua_getglobal(state, "world");
  lua_getfield(state, -1, "access");
  lua_getfield(state, -1, name);
  lua_remove(state, -2);
  lua_remove(state, -2);
}

static bool read_access(lua_State *state, const char *name,
                        LuaCommandAccess expected) {
  LuaCommandAccess access = LUA_COMMAND_ACCESS_COUNT;
  bool valid;

  lua_newtable(state);
  if (name != nullptr) {
    push_access(state, name);
    lua_setfield(state, -2, "access");
  }
  valid = lua_command_access_read(state, lua_gettop(state), &access);
  lua_pop(state, 1);
  return valid && access == expected;
}

static bool rejects_string_access(lua_State *state, const char *value) {
  LuaCommandAccess access = LUA_COMMAND_ACCESS_COUNT;

  lua_newtable(state);
  lua_pushstring(state, value);
  lua_setfield(state, -2, "access");
  bool valid = lua_command_access_read(state, lua_gettop(state), &access);
  lua_pop(state, 1);
  return !valid;
}

static int access_unknown(lua_State *state) {
  push_access(state, "UNKNOWN");
  return 1;
}

static int access_non_string(lua_State *state) {
  lua_getglobal(state, "world");
  lua_getfield(state, -1, "access");
  lua_pushinteger(state, 1);
  lua_gettable(state, -2);
  return 1;
}

static int access_namespace_mutate(lua_State *state) {
  lua_getglobal(state, "world");
  lua_getfield(state, -1, "access");
  lua_pushboolean(state, 1);
  lua_setfield(state, -2, "PUBLIC");
  return 0;
}

static int access_constant_mutate(lua_State *state) {
  push_access(state, "PUBLIC");
  lua_pushboolean(state, 1);
  lua_setfield(state, -2, "value");
  return 0;
}

static bool raises_access_invalid(lua_State *state, lua_CFunction function) {
  lua_pushcfunction(state, function);
  int status = lua_pcall(state, 0, 0, 0);
  const char *code = status ? lua_error_check_code(state, -1) : nullptr;
  bool expected = code != nullptr && !strcmp(code, "mux.access.invalid");

  if (status)
    lua_pop(state, 1);
  return status != 0 && expected;
}

static int command_handler(lua_State *state [[maybe_unused]]) { return 0; }

static bool read_entry(lua_State *state, GameDatabase *database, DbRef player,
                       const char *access_name, bool include_pattern,
                       bool include_handler) {
  const char *pattern = nullptr;
  bool readable;

  lua_newtable(state);
  if (access_name != nullptr) {
    push_access(state, access_name);
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

static bool rejects_string_entry(lua_State *state, GameDatabase *database,
                                 const char *access) {
  const char *pattern = nullptr;

  lua_newtable(state);
  lua_pushstring(state, access);
  lua_setfield(state, -2, "access");
  lua_pushliteral(state, "^test$");
  lua_setfield(state, -2, "pattern");
  lua_pushcfunction(state, command_handler);
  lua_setfield(state, -2, "handler");
  bool readable =
      lua_command_entry_read(state, lua_gettop(state), database, GOD, &pattern);
  lua_pop(state, 1);
  return !readable;
}

int main(void) {
  lua_State *state = luaL_newstate();
  GameObject objects[5] = {0};
  GameDatabase database = {.object_storage = objects, .top = 4, .size = 4};
  LuaCommandAccess access;

  if (state == nullptr)
    return 2;
  lua_newtable(state);
  lua_command_access_install_namespace(state);
  lua_setglobal(state, "world");
  game_database_object(&database, GOD)->type = OBJECT_TYPE_PLAYER;
  game_database_object(&database, GOD)->has_wizard_flag = true;
  game_database_object(&database, 2)->type = OBJECT_TYPE_PLAYER;
  game_database_object(&database, 2)->has_wizard_flag = true;
  game_database_object(&database, 3)->type = OBJECT_TYPE_PLAYER;

  if (!read_access(state, nullptr, LUA_COMMAND_ACCESS_PUBLIC) ||
      !read_access(state, "PUBLIC", LUA_COMMAND_ACCESS_PUBLIC) ||
      !read_access(state, "WIZARD", LUA_COMMAND_ACCESS_WIZARD) ||
      !read_access(state, "GOD", LUA_COMMAND_ACCESS_GOD) ||
      !rejects_string_access(state, "public") ||
      !rejects_string_access(state, "wizard") ||
      !rejects_string_access(state, "god")) {
    lua_close(state);
    return 1;
  }
  push_access(state, "PUBLIC");
  push_access(state, "PUBLIC");
  bool constants_equal = lua_equal(state, -1, -2) != 0;
  bool has_string = luaL_callmeta(state, -1, "__tostring") != 0;
  const char *constant_name = has_string ? lua_tostring(state, -1) : nullptr;
  bool constant_stringifies =
      constant_name != nullptr && !strcmp(constant_name, "PUBLIC");
  lua_pop(state, has_string ? 3 : 2);
  if (!constants_equal || !constant_stringifies ||
      !raises_access_invalid(state, access_unknown) ||
      !raises_access_invalid(state, access_non_string) ||
      !raises_access_invalid(state, access_namespace_mutate) ||
      !raises_access_invalid(state, access_constant_mutate)) {
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
      !read_entry(state, &database, 3, "PUBLIC", true, true) ||
      read_entry(state, &database, 3, "WIZARD", true, true) ||
      !read_entry(state, &database, 2, "WIZARD", true, true) ||
      read_entry(state, &database, 2, "GOD", true, true) ||
      !read_entry(state, &database, GOD, "GOD", true, true) ||
      read_entry(state, &database, GOD, "GOD", false, true) ||
      read_entry(state, &database, GOD, "GOD", true, false) ||
      !rejects_string_entry(state, &database, "public") ||
      !rejects_string_entry(state, &database, "wizard") ||
      !rejects_string_entry(state, &database, "god")) {
    lua_close(state);
    return 1;
  }

  lua_close(state);
  return 0;
}
