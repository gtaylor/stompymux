/* btech_character_bindings.c - Lua bindings for btech.character. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

/**
 * @par LuaLS definition btech alias btech.character.ref
 * @code{.lua}
 * ---@alias CharacterRef integer|string Character dbref or legacy character reference.
 * @endcode
 *
 * @par LuaLS definition btech alias btech.character.list-kind
 * @code{.lua}
 * ---@alias CharacterListKind "skills"|"advantages"|"attributes" Canonical character-value category; native matching is ASCII-case-insensitive.
 * @endcode
 *
 * @par LuaLS definition btech alias btech.character.value-mode
 * @code{.lua}
 * ---@alias CharacterValueMode 0|1|2|3|4 Legacy character-value lookup or mutation mode.
 * @endcode
 *
 * @par LuaLS definition btech namespace btech.character
 * @code{.lua}
 * ---Character values, skills, experience, and piloting rolls.
 * ---@class BtechCharacterPackage
 * local btech_character = {}
 * @endcode
 */
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
