/* btech_package.c - Lua bindings for the BattleTech host API. */

#include <lauxlib.h>
#include <lua.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

static void btech_lua_push_list(lua_State *state, const BtechScriptList *list) {
  lua_newtable(state);
  for (size_t index = 0; index < list->count; index++) {
    const BtechScriptListItem *item = checked_storage_at_const(
        list->items, list->count, sizeof(*list->items), index);
    if (item->kind == BTECH_SCRIPT_LIST_NUMBER)
      lua_pushinteger(state, item->value.number);
    else
      lua_pushstring(state, item->value.text);
    lua_rawseti(state, -2, (int)index + 1);
  }
}

static int btech_lua_invoke(lua_State *state) {
  LuaBtechPackage *package = lua_touserdata(state, lua_upvalueindex(1));
  const BtechLuaEntry *const *binding =
      (const BtechLuaEntry *const *)lua_touserdata(state, lua_upvalueindex(2));
  const BtechLuaEntry *entry = *binding;
  int argument_count = lua_gettop(state);
  char *arguments[MAX_ARG] = {};
  char *buffer = alloc_lbuf("btech_lua_invoke");
  char *cursor = buffer;

  if (package->is_checking && package->is_checking(package->context)) {
    free_buf(buffer);
    return lua_error_raise(state, LUA_ERROR_CODE_BTECH_UNAVAILABLE,
                           "btech.%s is unavailable during @lua/check",
                           entry->qualified_name);
  }
  if (argument_count > MAX_ARG) {
    free_buf(buffer);
    return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                           "too many arguments");
  }
  for (int index = 0; index < argument_count; index++) {
    const char *value;
    if (lua_isboolean(state, index + 1))
      value = lua_toboolean(state, index + 1) ? "1" : "0";
    else
      value = luaL_checkstring(state, index + 1);
    char **slot = (char **)checked_storage_at(
        (void *)arguments, MAX_ARG, sizeof(*arguments), (size_t)index);

    *slot = alloc_lbuf("btech_lua_argument");
    (void)string_copy_bounded(*slot, LBUF_SIZE, value);
  }
  BtechScriptCall call = {
      .evaluation = &package->services->background_command->evaluation,
      .player = GOD,
      .cause = GOD,
      .output = {.buffer = buffer, .cursor = cursor, .capacity = LBUF_SIZE},
      .arguments = {.values = arguments, .count = (size_t)argument_count},
  };
  BtechScriptResult result = entry->handler(&call);
  for (int index = 0; index < argument_count; index++)
    free_buf(*(char *const *)checked_storage_at_const(
        (const void *)arguments, MAX_ARG, sizeof(*arguments), (size_t)index));
  if (result.status == BTECH_SCRIPT_ERROR) {
    char error[LBUF_SIZE];
    (void)snprintf(error, sizeof(error), "%s", result.value.text);
    btech_script_result_destroy(&result);
    free_buf(buffer);
    return lua_error_raise(state, LUA_ERROR_CODE_BTECH_FAILED, "%s", error);
  }
  switch (result.kind) {
  case BTECH_SCRIPT_NUMBER:
    lua_pushnumber(state, result.value.number);
    break;
  case BTECH_SCRIPT_BOOLEAN:
    lua_pushboolean(state, result.value.boolean);
    break;
  case BTECH_SCRIPT_LIST:
    btech_lua_push_list(state, &result.value.list);
    break;
  case BTECH_SCRIPT_MUTATION:
    lua_pushboolean(state, result.value.mutation);
    break;
  case BTECH_SCRIPT_TEXT:
    lua_pushstring(state, result.value.text);
    break;
  }
  btech_script_result_destroy(&result);
  free_buf(buffer);
  return 1;
}

void lua_btech_install_bindings(lua_State *state, LuaBtechPackage *package,
                                const char *name, const BtechLuaEntry *entries,
                                size_t entry_count) {
  lua_newtable(state);
  for (size_t index = 0; index < entry_count; index++) {
    const BtechLuaEntry *entry =
        checked_storage_at_const(entries, entry_count, sizeof(*entries), index);
    const BtechLuaEntry **binding;

    lua_pushlightuserdata(state, package);
    binding = (const BtechLuaEntry **)lua_newuserdata(state, sizeof(*binding));
    *binding = entry;
    lua_pushcclosure(state, btech_lua_invoke, 2);
    lua_setfield(state, -2, entry->name);
  }
  lua_setfield(state, -2, name);
}

/**
 * Installs the root BattleTech package and its public namespaces.
 *
 * @par LuaLS definition btech alias btech.list-item
 * @code{.lua}
 * ---@alias BtechListItem string|number Item returned by a legacy BattleTech list result.
 * @endcode
 *
 * @par LuaLS definition btech namespace btech.package
 * @code{.lua}
 * ---The native BattleTech host API. All functions are unavailable during `@lua/check`.
 * ---@class BtechPackage
 * ---@field unit BtechUnitPackage Live units, templates, combat values, and status.
 * ---@field map BtechMapPackage Maps, geometry, line of sight, and map messaging.
 * ---@field parts BtechPartsPackage Part catalogues, installed parts, stores, and costs.
 * ---@field character BtechCharacterPackage Character values, skills, experience, and piloting rolls.
 * ---@field repair BtechRepairPackage Damage and technician-status queries.
 * ---@field system BtechSystemPackage Special-object fields and server-wide queries.
 * ---@field error BtechErrorPackage BattleTech checked error codes.
 * btech = {}
 * @endcode
 */
void lua_btech_package_install(lua_State *state, LuaBtechPackage *package) {
  lua_newtable(state);
  lua_btech_install_unit_bindings(state, package);
  lua_btech_install_map_bindings(state, package);
  lua_btech_install_parts_bindings(state, package);
  lua_btech_install_character_bindings(state, package);
  lua_btech_install_repair_bindings(state, package);
  lua_btech_install_system_bindings(state, package);
  lua_newtable(state);
  if (!lua_error_push_code_tree(state, "btech")) {
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "native btech error code tree is unavailable");
    return;
  }
  lua_setfield(state, -2, "codes");
  lua_setfield(state, -2, "error");
  lua_setglobal(state, "btech");
}
