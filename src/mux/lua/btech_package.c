/* btech_package.c - Lua bindings for the BattleTech host API. */

#include <lauxlib.h>
#include <lua.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/scripting/script_functions_api.h"
#include "mux/lua/btech_package.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

typedef struct BtechLuaEntry BtechLuaEntry;
struct BtechLuaEntry {
  const char *name;
  BtechScriptFunction *handler;
};

static BtechLuaEntry btech_lua_entries[] = {
    {"add_stores", fun_btaddstores},
    {"armor_status", fun_btarmorstatus},
    {"armor_status_ref", fun_btarmorstatus_ref},
    {"char_list", fun_btcharlist},
    {"crit_slot", fun_btcritslot},
    {"crit_slot_ref", fun_btcritslot_ref},
    {"section_status", fun_btsectstatus},
    {"crit_status", fun_btcritstatus},
    {"crit_status_ref", fun_btcritstatus_ref},
    {"damage_mech", fun_btdamagemech},
    {"damages", fun_btdamages},
    {"design_exists", fun_btdesignex},
    {"engine_rating", fun_btengrate},
    {"engine_rating_ref", fun_btengrate_ref},
    {"fasa_base_cost_ref", fun_btfasabasecost_ref},
    {"battle_value", fun_btgetbv},
    {"battle_value_ref", fun_btgetbv_ref},
    {"battle_value2_ref", fun_btgetbv2_ref},
    {"defensive_battle_value_ref", fun_btgetdbv_ref},
    {"offensive_battle_value_ref", fun_btgetobv_ref},
    {"char_value", fun_btgetcharvalue},
    {"part_cost", fun_btgetpartcost},
    {"range", fun_btgetrange},
    {"real_max_speed", fun_btgetrealmaxspeed},
    {"get_weight", fun_btgetweight},
    {"xcode_value", fun_btgetxcodevalue},
    {"xcode_value_ref", fun_btgetxcodevalue_ref},
    {"hex_emit", fun_bthexemit},
    {"hex_in_blast_zone", fun_bthexinblz},
    {"hex_line_of_sight", fun_bthexlos},
    {"id_to_dbref", fun_btid2db},
    {"lag", fun_btlag},
    {"blast_zones", fun_btlistblz},
    {"load_map", fun_btloadmap},
    {"load_mech", fun_btloadmech},
    {"mech_line_of_sight", fun_btlosm2m},
    {"make_pilot_roll", fun_btmakepilotroll},
    {"map_elevation", fun_btmapelev},
    {"map_emit", fun_btmapemit},
    {"map_terrain", fun_btmapterr},
    {"map_units", fun_btmapunits},
    {"mech_frequencies", fun_btmechfreqs},
    {"repair_job_count", fun_btnumrepjobs},
    {"part_type", fun_btparttype},
    {"part_match", fun_btpartmatch},
    {"part_name", fun_btpartname},
    {"part_categories", fun_btpartscategorylist},
    {"parts", fun_btpartslist},
    {"part_weight", fun_btgetweight},
    {"payload_ref", fun_btpayload_ref},
    {"remove_stores", fun_btremovestores},
    {"set_armor_status", fun_btsetarmorstatus},
    {"set_char_value", fun_btsetcharvalue},
    {"set_max_speed", fun_btsetmaxspeed},
    {"set_part_cost", fun_btsetpartcost},
    {"set_tons", fun_btsettons},
    {"set_xcode_value", fun_btsetxcodevalue},
    {"set_xy", fun_btsetxy},
    {"show_crit_status_ref", fun_btshowcritstatus_ref},
    {"show_status_ref", fun_btshowstatus_ref},
    {"show_weapon_specs_ref", fun_btshowwspecs_ref},
    {"stores", fun_btstores},
    {"stores_short", fun_btstores_short},
    {"tech_list", fun_bttechlist},
    {"tech_list_ref", fun_bttechlist_ref},
    {"tech_status", fun_bttechstatus},
    {"tech_time", fun_bttechtime},
    {"threshold", fun_btthreshold},
    {"tic_weapons", fun_btticweaps},
    {"under_repair", fun_btunderrepair},
    {"unit_fixable", fun_btunitfixable},
    {"unit_parts", fun_btunitpartslist},
    {"unit_parts_ref", fun_btunitpartslist_ref},
    {"update_links", fun_btupdatelinks},
    {"weapon_status", fun_btweaponstatus},
    {"weapon_status_ref", fun_btweaponstatus_ref},
    {"weapon_stat", fun_btweapstat},
    {"zone_mechs", fun_zmechs},
    {nullptr, nullptr},
};

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
  BtechLuaEntry *entry = lua_touserdata(state, lua_upvalueindex(2));
  int argument_count = lua_gettop(state);
  char *arguments[MAX_ARG] = {0};
  char *buffer = alloc_lbuf("btech_lua_invoke");
  char *cursor = buffer;

  if (package->is_checking && package->is_checking(package->context)) {
    free_lbuf(buffer);
    return luaL_error(state, "btech.%s is unavailable during @lua/check",
                      entry->name);
  }
  if (argument_count > MAX_ARG) {
    free_lbuf(buffer);
    return luaL_error(state, "too many arguments");
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
    StringCopy(*slot, value);
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
    free_lbuf(*(char *const *)checked_storage_at_const(
        (const void *)arguments, MAX_ARG, sizeof(*arguments), (size_t)index));
  if (result.status == BTECH_SCRIPT_ERROR) {
    char error[LBUF_SIZE];
    (void)snprintf(error, sizeof(error), "%s", result.value.text);
    btech_script_result_destroy(&result);
    free_lbuf(buffer);
    return luaL_error(state, "%s", error);
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
  free_lbuf(buffer);
  return 1;
}

void lua_btech_package_install(lua_State *state, LuaBtechPackage *package) {
  lua_newtable(state);
  constexpr size_t entry_count =
      sizeof(btech_lua_entries) / sizeof(btech_lua_entries[0]) - 1;

  for (size_t index = 0; index < entry_count; index++) {
    BtechLuaEntry *entry = checked_storage_at(
        btech_lua_entries, entry_count, sizeof(*btech_lua_entries), index);

    lua_pushlightuserdata(state, package);
    lua_pushlightuserdata(state, entry);
    lua_pushcclosure(state, btech_lua_invoke, 2);
    lua_setfield(state, -2, entry->name);
  }
  lua_setglobal(state, "btech");
}
