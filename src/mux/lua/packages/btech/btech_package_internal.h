/* Private interfaces shared by built-in btech package binding modules. */

#pragma once

#include <stddef.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"

typedef struct BtechLuaEntry BtechLuaEntry;
struct BtechLuaEntry {
  const char *name;
  const char *qualified_name;
  BtechScriptFunction *handler;
};

void lua_btech_install_bindings(lua_State *state, LuaBtechPackage *package,
                                const char *name, const BtechLuaEntry *entries,
                                size_t entry_count);
void lua_btech_install_unit_bindings(lua_State *state,
                                     LuaBtechPackage *package);
void lua_btech_install_map_bindings(lua_State *state, LuaBtechPackage *package);
void lua_btech_install_parts_bindings(lua_State *state,
                                      LuaBtechPackage *package);
void lua_btech_install_character_bindings(lua_State *state,
                                          LuaBtechPackage *package);
void lua_btech_install_repair_bindings(lua_State *state,
                                       LuaBtechPackage *package);
void lua_btech_install_system_bindings(lua_State *state,
                                       LuaBtechPackage *package);
