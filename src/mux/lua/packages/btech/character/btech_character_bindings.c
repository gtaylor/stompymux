/* btech_character_bindings.c - Lua bindings for btech.character. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

static const BtechLuaEntry BTECH_CHARACTER_ENTRIES[] = {
    {"list", "character.list", fun_btcharlist},
    {"set_value", "character.set_value", fun_btsetcharvalue},
    {"threshold", "character.threshold", fun_btthreshold},
    {"value", "character.value", fun_btgetcharvalue},
};

void lua_btech_install_character_bindings(lua_State *state,
                                          LuaBtechPackage *package) {
  lua_btech_install_bindings(
      state, package, "character", BTECH_CHARACTER_ENTRIES,
      sizeof(BTECH_CHARACTER_ENTRIES) / sizeof(BTECH_CHARACTER_ENTRIES[0]));
}
