/* Private interfaces shared by built-in btech package binding modules. */

#pragma once

#include <stddef.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"

typedef struct BtechLuaEntry BtechLuaEntry;
typedef int BtechLuaNativeFunction(lua_State *state, LuaBtechPackage *package);
typedef struct BtechLuaNativeEntry BtechLuaNativeEntry;
struct BtechLuaEntry {
  const char *name;
  const char *qualified_name;
  BtechScriptFunction *handler;
};

struct BtechLuaNativeEntry {
  const char *name;
  const char *qualified_name;
  BtechLuaNativeFunction *handler;
};

void lua_btech_install_bindings(lua_State *state, LuaBtechPackage *package,
                                const char *name, const BtechLuaEntry *entries,
                                size_t entry_count);
void lua_btech_install_native_bindings(lua_State *state,
                                       LuaBtechPackage *package,
                                       const char *name,
                                       const BtechLuaNativeEntry *entries,
                                       size_t entry_count);
void lua_btech_check_arity(lua_State *state, int expected);
void lua_btech_check_options(lua_State *state, int table,
                             const char *const *allowed, size_t allowed_count,
                             int argument);
void lua_btech_get_field(lua_State *state, int table, const char *field);
long lua_btech_check_integer_field(lua_State *state, int table,
                                   const char *field, long minimum,
                                   long maximum, int argument);
bool lua_btech_check_boolean_field(lua_State *state, int table,
                                   const char *field, int argument);
const char *lua_btech_check_string_field(lua_State *state, int table,
                                         const char *field, size_t maximum,
                                         int argument);
DbRef lua_btech_require_object(LuaBtechPackage *package, lua_State *state,
                               int argument);
DbRef lua_btech_require_object_field(LuaBtechPackage *package, lua_State *state,
                                     int table, const char *field,
                                     int argument);
void lua_btech_push_object(lua_State *state, LuaBtechPackage *package,
                           DbRef object);
struct BtechContext *lua_btech_context(LuaBtechPackage *package);
void lua_btech_install_unit_bindings(lua_State *state,
                                     LuaBtechPackage *package);
void lua_btech_install_map_bindings(lua_State *state, LuaBtechPackage *package);
void lua_btech_install_player_bindings(lua_State *state,
                                       LuaBtechPackage *package);
void lua_btech_install_parts_bindings(lua_State *state,
                                      LuaBtechPackage *package);
void lua_btech_install_character_bindings(lua_State *state,
                                          LuaBtechPackage *package);
void lua_btech_install_repair_bindings(lua_State *state,
                                       LuaBtechPackage *package);
void lua_btech_install_system_bindings(lua_State *state,
                                       LuaBtechPackage *package);
