/* btech_unit_bindings.c - Lua bindings for btech.unit. */

#include <lua.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"

static const BtechLuaEntry BTECH_UNIT_ENTRIES[] = {
    {"armor_status", "unit.armor_status", fun_btarmorstatus},
    {"armor_status_ref", "unit.armor_status_ref", fun_btarmorstatus_ref},
    {"battle_value", "unit.battle_value", fun_btgetbv},
    {"battle_value_ref", "unit.battle_value_ref", fun_btgetbv_ref},
    {"battle_value2_ref", "unit.battle_value2_ref", fun_btgetbv2_ref},
    {"crit_slot", "unit.crit_slot", fun_btcritslot},
    {"crit_slot_ref", "unit.crit_slot_ref", fun_btcritslot_ref},
    {"crit_status", "unit.crit_status", fun_btcritstatus},
    {"crit_status_ref", "unit.crit_status_ref", fun_btcritstatus_ref},
    {"damage", "unit.damage", fun_btdamagemech},
    {"defensive_battle_value_ref", "unit.defensive_battle_value_ref",
     fun_btgetdbv_ref},
    {"engine_rating", "unit.engine_rating", fun_btengrate},
    {"engine_rating_ref", "unit.engine_rating_ref", fun_btengrate_ref},
    {"fasa_base_cost_ref", "unit.fasa_base_cost_ref", fun_btfasabasecost_ref},
    {"frequencies", "unit.frequencies", fun_btmechfreqs},
    {"load", "unit.load", fun_btloadmech},
    {"make_pilot_roll", "unit.make_pilot_roll", fun_btmakepilotroll},
    {"offensive_battle_value_ref", "unit.offensive_battle_value_ref",
     fun_btgetobv_ref},
    {"payload_ref", "unit.payload_ref", fun_btpayload_ref},
    {"real_max_speed", "unit.real_max_speed", fun_btgetrealmaxspeed},
    {"section_status", "unit.section_status", fun_btsectstatus},
    {"set_armor_status", "unit.set_armor_status", fun_btsetarmorstatus},
    {"set_max_speed", "unit.set_max_speed", fun_btsetmaxspeed},
    {"set_tons", "unit.set_tons", fun_btsettons},
    {"show_crit_status_ref", "unit.show_crit_status_ref",
     fun_btshowcritstatus_ref},
    {"show_status_ref", "unit.show_status_ref", fun_btshowstatus_ref},
    {"show_weapon_specs_ref", "unit.show_weapon_specs_ref",
     fun_btshowwspecs_ref},
    {"tic_weapons", "unit.tic_weapons", fun_btticweaps},
    {"weapon_status", "unit.weapon_status", fun_btweaponstatus},
    {"weapon_status_ref", "unit.weapon_status_ref", fun_btweaponstatus_ref},
};

void lua_btech_install_unit_bindings(lua_State *state,
                                     LuaBtechPackage *package) {
  lua_btech_install_bindings(state, package, "unit", BTECH_UNIT_ENTRIES,
                             sizeof(BTECH_UNIT_ENTRIES) /
                                 sizeof(BTECH_UNIT_ENTRIES[0]));
}
