/* command_access.c - Access levels for Lua command entries. */

#include "mux/lua/command_access.h"

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static const char LUA_COMMAND_ACCESS_METATABLE[] = "btmux.command_access";
static const char LUA_COMMAND_ACCESS_NAMESPACE_METATABLE[] =
    "btmux.command_access_namespace";

typedef struct LuaCommandAccessConstant {
  LuaCommandAccess access;
  const char *name;
} LuaCommandAccessConstant;

typedef struct LuaCommandAccessEntry {
  LuaCommandAccess access;
  const char *name;
} LuaCommandAccessEntry;

static const LuaCommandAccessEntry LUA_COMMAND_ACCESS_ENTRIES[] = {
    {LUA_COMMAND_ACCESS_PUBLIC, "PUBLIC"},
    {LUA_COMMAND_ACCESS_WIZARD, "WIZARD"},
    {LUA_COMMAND_ACCESS_GOD, "GOD"},
};

static const LuaCommandAccessEntry *
lua_command_access_entry_at(LuaCommandAccess access) {
  if (access < 0 || access >= LUA_COMMAND_ACCESS_COUNT)
    return nullptr;
  return checked_storage_at_const(
      LUA_COMMAND_ACCESS_ENTRIES,
      sizeof(LUA_COMMAND_ACCESS_ENTRIES) / sizeof(*LUA_COMMAND_ACCESS_ENTRIES),
      sizeof(*LUA_COMMAND_ACCESS_ENTRIES), (size_t)access);
}

static void lua_command_access_push(lua_State *state,
                                    const LuaCommandAccessEntry *entry) {
  LuaCommandAccessConstant *constant =
      lua_newuserdata(state, sizeof(*constant));

  *constant = (LuaCommandAccessConstant){
      .access = entry->access,
      .name = entry->name,
  };
  luaL_getmetatable(state, LUA_COMMAND_ACCESS_METATABLE);
  lua_setmetatable(state, -2);
}

static int lua_command_access_tostring(lua_State *state) {
  LuaCommandAccessConstant *constant =
      luaL_checkudata(state, 1, LUA_COMMAND_ACCESS_METATABLE);

  lua_pushstring(state, constant->name);
  return 1;
}

static int lua_command_access_equal(lua_State *state) {
  LuaCommandAccessConstant *left =
      luaL_checkudata(state, 1, LUA_COMMAND_ACCESS_METATABLE);
  LuaCommandAccessConstant *right =
      luaL_checkudata(state, 2, LUA_COMMAND_ACCESS_METATABLE);

  lua_pushboolean(state, left->access == right->access);
  return 1;
}

static int lua_command_access_newindex(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ACCESS_INVALID,
                         "command access constants are immutable");
}

static int lua_command_access_namespace_index(lua_State *state) {
  if (lua_type(state, 2) != LUA_TSTRING)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ACCESS_INVALID,
                         "constant name must be a string");
  const char *name = lua_tostring(state, 2);

  for (LuaCommandAccess access = LUA_COMMAND_ACCESS_PUBLIC;
       access < LUA_COMMAND_ACCESS_COUNT; access++) {
    const LuaCommandAccessEntry *entry = lua_command_access_entry_at(access);

    if (!strcmp(name, entry->name)) {
      lua_command_access_push(state, entry);
      return 1;
    }
  }
  return lua_error_arg(state, 2, LUA_ERROR_CODE_ACCESS_INVALID,
                       "unknown command access constant '%s'", name);
}

static int lua_command_access_namespace_newindex(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ACCESS_INVALID,
                         "mux.world.access constants are immutable");
}

bool lua_command_access_read(lua_State *state, int entry,
                             LuaCommandAccess *access) {
  bool valid;

  lua_getfield(state, entry, "access");
  if (lua_isnil(state, -1)) {
    *access = LUA_COMMAND_ACCESS_PUBLIC;
    valid = true;
  } else {
    LuaCommandAccessConstant *constant =
        luaL_testudata(state, -1, LUA_COMMAND_ACCESS_METATABLE);

    valid = (bool)(constant != nullptr &&
                   lua_command_access_entry_at(
                       constant ? constant->access
                                : LUA_COMMAND_ACCESS_COUNT) != nullptr);
    if (valid)
      *access = constant->access;
  }
  lua_pop(state, 1);
  return valid;
}

/**
 * Installs typed command-access constants.
 *
 * @par Lua name `mux.world.access`
 * @par Lua constants - `PUBLIC` (`Access`) Allows every invoker.
 * - `WIZARD` (`Access`) Allows Wizards and God.
 * - `GOD` (`Access`) Allows only God.
 * @par Lua errors - `LUA_ERROR_CODE_ACCESS_INVALID` for unknown or non-string
 * lookups and attempted mutation.
 * @param[in,out] state Lua state whose top value is the `mux.world` table.
 */
void lua_command_access_install_namespace(lua_State *state) {
  luaL_newmetatable(state, LUA_COMMAND_ACCESS_METATABLE);
  lua_pushcfunction(state, lua_command_access_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_command_access_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_command_access_newindex);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_COMMAND_ACCESS_NAMESPACE_METATABLE);
  lua_pushcfunction(state, lua_command_access_namespace_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_command_access_namespace_newindex);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  (void)lua_newuserdata(state, 1);
  luaL_getmetatable(state, LUA_COMMAND_ACCESS_NAMESPACE_METATABLE);
  lua_setmetatable(state, -2);
  lua_setfield(state, -2, "access");
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
