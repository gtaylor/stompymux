/* btech_package.c - Lua bindings for the BattleTech host API. */

#include <lauxlib.h>
#include <limits.h>
#include <lua.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/context.h"
#include "btech/scripting/script_functions_api.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
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
    lua_pushboolean(state, (int)result.value.boolean);
    break;
  case BTECH_SCRIPT_LIST:
    btech_lua_push_list(state, &result.value.list);
    break;
  case BTECH_SCRIPT_MUTATION:
    lua_pushboolean(state, (int)result.value.mutation);
    break;
  case BTECH_SCRIPT_TEXT:
    lua_pushstring(state, result.value.text);
    break;
  }
  btech_script_result_destroy(&result);
  free_buf(buffer);
  return 1;
}

static int btech_lua_invoke_native(lua_State *state) {
  LuaBtechPackage *package = lua_touserdata(state, lua_upvalueindex(1));
  const BtechLuaNativeEntry *const *binding =
      (const BtechLuaNativeEntry *const *)lua_touserdata(state,
                                                         lua_upvalueindex(2));
  const BtechLuaNativeEntry *entry = *binding;

  if (package->is_checking && package->is_checking(package->context))
    return lua_error_raise(state, LUA_ERROR_CODE_BTECH_UNAVAILABLE,
                           "btech.%s is unavailable during @lua/check",
                           entry->qualified_name);
  return entry->handler(state, package);
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

void lua_btech_install_native_bindings(lua_State *state,
                                       LuaBtechPackage *package,
                                       const char *name,
                                       const BtechLuaNativeEntry *entries,
                                       size_t entry_count) {
  lua_getfield(state, -1, name);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    lua_newtable(state);
  }
  for (size_t index = 0; index < entry_count; index++) {
    const BtechLuaNativeEntry *entry =
        checked_storage_at_const(entries, entry_count, sizeof(*entries), index);
    const BtechLuaNativeEntry **binding;

    lua_pushlightuserdata(state, package);
    binding =
        (const BtechLuaNativeEntry **)lua_newuserdata(state, sizeof(*binding));
    *binding = entry;
    lua_pushcclosure(state, btech_lua_invoke_native, 2);
    lua_setfield(state, -2, entry->name);
  }
  lua_setfield(state, -2, name);
}

void lua_btech_check_arity(lua_State *state, int expected) {
  if (lua_gettop(state) != expected)
    (void)lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                          "expected %d arguments", expected);
}

static bool option_allowed(const char *field, const char *const *allowed,
                           size_t allowed_count) {
  for (size_t index = 0; index < allowed_count; index++)
    if (strcmp(field, *(const char *const *)checked_storage_at_const(
                          (const void *)allowed, allowed_count,
                          sizeof(*allowed), index)) == 0)
      return true;
  return false;
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters): Lua stack positions and
// public argument positions are intentionally distinct integer coordinates.
void lua_btech_check_options(lua_State *state, int table,
                             const char *const *allowed, size_t allowed_count,
                             int argument) {
  if (!lua_istable(state, table))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "value must be a table");
  if (table < 0)
    table = lua_gettop(state) + table + 1;
  lua_pushnil(state);
  while (lua_next(state, table) != 0) {
    const char *field =
        lua_type(state, -2) == LUA_TSTRING ? lua_tostring(state, -2) : nullptr;
    lua_pop(state, 1);
    if (field == nullptr || !option_allowed(field, allowed, allowed_count))
      (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                          "unknown field '%s'",
                          field == nullptr ? "<non-string>" : field);
  }
}

void lua_btech_get_field(lua_State *state, int table, const char *field) {
  if (table < 0)
    table = lua_gettop(state) + table + 1;
  lua_pushstring(state, field);
  lua_rawget(state, table);
}

long lua_btech_check_integer_field(lua_State *state, int table,
                                   const char *field, long minimum,
                                   long maximum, int argument) {
  lua_btech_get_field(state, table, field);
  if (!lua_isnumber(state, -1))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be an integer", field);
  const lua_Number NUMBER = lua_tonumber(state, -1);
  if (!isfinite(NUMBER) || floor(NUMBER) != NUMBER ||
      NUMBER < (lua_Number)minimum || NUMBER > (lua_Number)maximum)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be an integer from %ld to %ld", field, minimum,
                        maximum);
  lua_pop(state, 1);
  return (long)NUMBER;
}

bool lua_btech_check_boolean_field(lua_State *state, int table,
                                   const char *field, int argument) {
  lua_btech_get_field(state, table, field);
  if (!lua_isboolean(state, -1))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a boolean", field);
  const bool VALUE = lua_toboolean(state, -1) != 0;
  lua_pop(state, 1);
  return VALUE;
}

const char *lua_btech_check_string_field(lua_State *state, int table,
                                         const char *field, size_t maximum,
                                         int argument) {
  lua_btech_get_field(state, table, field);
  if (lua_type(state, -1) != LUA_TSTRING)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a string", field);
  size_t length;
  const char *value = lua_tolstring(state, -1, &length);
  if (length == 0 || length > maximum)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must contain 1 to %zu bytes", field, maximum);
  lua_pop(state, 1);
  return value;
}

DbRef lua_btech_require_object(LuaBtechPackage *package, lua_State *state,
                               int argument) {
  return lua_mux_require_object(package->mux_package, state, argument);
}

DbRef lua_btech_require_object_field(LuaBtechPackage *package, lua_State *state,
                                     int table, const char *field,
                                     int argument) {
  lua_btech_get_field(state, table, field);
  if (lua_isnil(state, -1))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s is required", field);
  const DbRef OBJECT = lua_mux_require_object_at(package->mux_package, state,
                                                 -1, argument, field);
  lua_pop(state, 1);
  return OBJECT;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

void lua_btech_push_object(lua_State *state, LuaBtechPackage *package,
                           DbRef object) {
  (void)lua_mux_push_object(state, package->mux_package, object);
}

BtechContext *lua_btech_context(LuaBtechPackage *package) {
  return package->services->background_command->evaluation.btech;
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
 * ---@field player BtechPlayerPackage Player-owned BattleTech configuration.
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
  lua_btech_install_player_bindings(state, package);
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
