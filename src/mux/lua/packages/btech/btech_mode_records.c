/* btech_mode_records.c - Structured critical-slot mode records for Lua. */

#include <lua.h>
#include <stdbool.h>
#include <stddef.h>

#include "btech/unit/template_api.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

void lua_btech_push_critical_modes(lua_State *state, unsigned int modes,
                                   bool ammunition) {
  const size_t COUNT = ammunition ? template_critical_ammo_mode_count()
                                  : template_critical_fire_mode_count();
  int output = 1;
  lua_newtable(state);
  for (size_t index = 0; index < COUNT; index++) {
    if ((modes & (1U << index)) == 0)
      continue;
    const char *name = ammunition ? template_critical_ammo_mode_name(index)
                                  : template_critical_fire_mode_name(index);
    lua_pushstring(state, name);
    lua_rawseti(state, -2, output++);
  }
}
