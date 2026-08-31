/* mux_options.c - Shared parsing for built-in Lua mux package options. */

#include <lua.h>

#include <stdio.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static bool lua_mux_option_allowed(const char *field,
                                   const char *const *allowed,
                                   size_t allowed_count) {
  for (size_t index = 0; index < allowed_count; index++) {
    const char *candidate = *(const char *const *)checked_storage_at_const(
        (const void *)allowed, allowed_count, sizeof(*allowed), index);

    if (!strcmp(field, candidate))
      return true;
  }
  return false;
}

void lua_mux_check_options(lua_State *state, int table,
                           const char *const *allowed, size_t allowed_count) {
  if (!lua_istable(state, table))
    lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                  "options must be a table");
  if (table < 0 && table > LUA_REGISTRYINDEX)
    table = lua_gettop(state) + table + 1;
  lua_pushnil(state);
  while (lua_next(state, table) != 0) {
    const char *field =
        lua_type(state, -2) == LUA_TSTRING ? lua_tostring(state, -2) : nullptr;

    lua_pop(state, 1);
    if (!field || !lua_mux_option_allowed(field, allowed, allowed_count))
      lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                    "unknown options field '%s'",
                    field ? field : "<non-string>");
  }
}

DbRef lua_mux_option_object(LuaMuxPackage *package, lua_State *state, int table,
                            const char *field, bool required, bool *present) {
  char label[64];

  lua_getfield(state, table, field);
  *present = lua_isnil(state, -1) == 0;
  if (!*present) {
    lua_pop(state, 1);
    if (required)
      return lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                           "options.%s is required", field);
    return NOTHING;
  }
  (void)snprintf(label, sizeof(label), "options.%s", field);
  DbRef object = lua_mux_require_object_at(package, state, -1, table, label);

  lua_pop(state, 1);
  return object;
}
