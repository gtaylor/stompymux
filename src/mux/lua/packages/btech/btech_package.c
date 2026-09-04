/* btech_package.c - Lua bindings for the BattleTech host API. */

#include <limits.h>
#include <lua.h>
#include <math.h>
#include <string.h>

#include "btech/context.h"
#include "btech/special_objects.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static int btech_lua_invoke_native(lua_State *state) {
  LuaBtechPackage *package = lua_touserdata(state, lua_upvalueindex(1));
  const BtechLuaNativeEntry *const *binding =
      (const BtechLuaNativeEntry *const *)lua_touserdata(state,
                                                         lua_upvalueindex(2));
  const BtechLuaNativeEntry *entry = *binding;

  if (package->is_checking && package->is_checking(package->context))
    return lua_error_raise(state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
                           "btech.%s is unavailable during @lua/check",
                           entry->qualified_name);
  return entry->handler(state, package);
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters): The machine-readable
// reason and human-readable message intentionally have the same C type.
int lua_btech_operation_error(lua_State *state, const char *reason,
                              const char *message) {
  lua_error_push(state,
                 lua_error_code_name(LUA_ERROR_CODE_BTECH_OPERATION_FAILED),
                 message);
  lua_newtable(state);
  lua_pushstring(state, reason);
  lua_setfield(state, -2, "reason");
  lua_setfield(state, -2, "detail");
  return lua_error(state);
}
// NOLINTEND(bugprone-easily-swappable-parameters)

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
  if (lua_gettop(state) < expected)
    (void)lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                          "expected at least %d arguments", expected);
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

void lua_btech_validate_resource_name(lua_State *state, int argument,
                                      const char *name, const char *label) {
  if (strstr(name, "..") != nullptr || strchr(name, '/') != nullptr ||
      strchr(name, '\\') != nullptr)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must not contain path components", label);
}

long lua_btech_check_integer_field(lua_State *state, int table,
                                   const char *field, long minimum,
                                   long maximum, int argument) {
  lua_btech_get_field(state, table, field);
  if (lua_type(state, -1) != LUA_TNUMBER)
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
  const DbRef OBJECT =
      lua_mux_require_object(package->mux_package, state, argument);
  if (is_going(package->services->database, OBJECT))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                        "object is going away");
  return OBJECT;
}

DbRef lua_btech_require_special(LuaBtechPackage *package, lua_State *state,
                                int argument, int type, const char *label) {
  const DbRef OBJECT = lua_btech_require_object(package, state, argument);
  if (btech_special_object_type(lua_btech_context(package), OBJECT) != type)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                        "object is not a registered BTech %s", label);
  return OBJECT;
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
  if (is_going(package->services->database, OBJECT))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                        "%s is going away", field);
  lua_pop(state, 1);
  return OBJECT;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

void lua_btech_push_object(lua_State *state, LuaBtechPackage *package,
                           DbRef object) {
  (void)lua_mux_push_object(state, package->mux_package, object);
}

void lua_btech_push_optional_object(lua_State *state, LuaBtechPackage *package,
                                    DbRef object) {
  if (object == NOTHING || (is_good_obj(package->services->database, object) &&
                            is_going(package->services->database, object))) {
    lua_pushnil(state);
    return;
  }
  if (!is_good_obj(package->services->database, object))
    (void)lua_error_raise(state, LUA_ERROR_CODE_OBJECT_INVALID,
                          "stored object relationship is corrupt");
  lua_btech_push_object(state, package, object);
}

BtechContext *lua_btech_context(LuaBtechPackage *package) {
  return package->services->background_command->evaluation.btech;
}

/**
 * Installs the root BattleTech package and its public namespaces.
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
  lua_btech_install_template_bindings(state, package);
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
