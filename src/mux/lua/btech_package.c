/* btech_package.c - Lua bindings for the BattleTech host API. */

#include "mux/server/platform.h"

#include "mux/lua/btech_package.h"

#include <lauxlib.h>

#include "mux/commands/command_context.h"
#include "mux/database/flags.h"
#include "mux/lua/lua_runtime.h"
#include "mux/support/alloc.h"

typedef void BtechLuaHandler(char *, char **, DbRef, DbRef, char **, int,
                             char **, int, EvaluationContext *);

extern BtechLuaHandler fun_btaddstores;
extern BtechLuaHandler fun_btarmorstatus;
extern BtechLuaHandler fun_btarmorstatus_ref;
extern BtechLuaHandler fun_btcharlist;
extern BtechLuaHandler fun_btcritslot;
extern BtechLuaHandler fun_btcritslot_ref;
extern BtechLuaHandler fun_btsectstatus;
extern BtechLuaHandler fun_btcritstatus;
extern BtechLuaHandler fun_btcritstatus_ref;
extern BtechLuaHandler fun_btdamagemech;
extern BtechLuaHandler fun_btdamages;
extern BtechLuaHandler fun_btdesignex;
extern BtechLuaHandler fun_btengrate;
extern BtechLuaHandler fun_btengrate_ref;
extern BtechLuaHandler fun_btfasabasecost_ref;
extern BtechLuaHandler fun_btgetbv;
extern BtechLuaHandler fun_btgetbv_ref;
extern BtechLuaHandler fun_btgetbv2_ref;
extern BtechLuaHandler fun_btgetdbv_ref;
extern BtechLuaHandler fun_btgetobv_ref;
extern BtechLuaHandler fun_btgetcharvalue;
extern BtechLuaHandler fun_btgetpartcost;
extern BtechLuaHandler fun_btgetrange;
extern BtechLuaHandler fun_btgetrealmaxspeed;
extern BtechLuaHandler fun_btgetweight;
extern BtechLuaHandler fun_btgetxcodevalue;
extern BtechLuaHandler fun_btgetxcodevalue_ref;
extern BtechLuaHandler fun_bthexemit;
extern BtechLuaHandler fun_bthexinblz;
extern BtechLuaHandler fun_bthexlos;
extern BtechLuaHandler fun_btid2db;
extern BtechLuaHandler fun_btlag;
extern BtechLuaHandler fun_btlistblz;
extern BtechLuaHandler fun_btloadmap;
extern BtechLuaHandler fun_btloadmech;
extern BtechLuaHandler fun_btlosm2m;
extern BtechLuaHandler fun_btmakepilotroll;
extern BtechLuaHandler fun_btmapelev;
extern BtechLuaHandler fun_btmapemit;
extern BtechLuaHandler fun_btmapterr;
extern BtechLuaHandler fun_btmapunits;
extern BtechLuaHandler fun_btmechfreqs;
extern BtechLuaHandler fun_btnumrepjobs;
extern BtechLuaHandler fun_btparttype;
extern BtechLuaHandler fun_btpartmatch;
extern BtechLuaHandler fun_btpartname;
extern BtechLuaHandler fun_btpartscategorylist;
extern BtechLuaHandler fun_btpartslist;
extern BtechLuaHandler fun_btpayload_ref;
extern BtechLuaHandler fun_btremovestores;
extern BtechLuaHandler fun_btsetarmorstatus;
extern BtechLuaHandler fun_btsetcharvalue;
extern BtechLuaHandler fun_btsetmaxspeed;
extern BtechLuaHandler fun_btsetpartcost;
extern BtechLuaHandler fun_btsettons;
extern BtechLuaHandler fun_btsetxcodevalue;
extern BtechLuaHandler fun_btsetxy;
extern BtechLuaHandler fun_btshowcritstatus_ref;
extern BtechLuaHandler fun_btshowstatus_ref;
extern BtechLuaHandler fun_btshowwspecs_ref;
extern BtechLuaHandler fun_btstores;
extern BtechLuaHandler fun_btstores_short;
extern BtechLuaHandler fun_bttechlist;
extern BtechLuaHandler fun_bttechlist_ref;
extern BtechLuaHandler fun_bttechstatus;
extern BtechLuaHandler fun_bttechtime;
extern BtechLuaHandler fun_btthreshold;
extern BtechLuaHandler fun_btticweaps;
extern BtechLuaHandler fun_btunderrepair;
extern BtechLuaHandler fun_btunitfixable;
extern BtechLuaHandler fun_btunitpartslist;
extern BtechLuaHandler fun_btunitpartslist_ref;
extern BtechLuaHandler fun_btupdatelinks;
extern BtechLuaHandler fun_btweaponstatus;
extern BtechLuaHandler fun_btweaponstatus_ref;
extern BtechLuaHandler fun_btweapstat;
extern BtechLuaHandler fun_zmechs;

typedef enum BtechLuaResult {
  BTECH_LUA_TEXT,
  BTECH_LUA_NUMBER,
  BTECH_LUA_BOOLEAN,
  BTECH_LUA_LIST,
  BTECH_LUA_MUTATION,
} BtechLuaResult;

typedef struct BtechLuaEntry BtechLuaEntry;
struct BtechLuaEntry {
  const char *name;
  BtechLuaHandler *handler;
  BtechLuaResult result;
};

static BtechLuaEntry btech_lua_entries[] = {
    {"add_stores", fun_btaddstores, BTECH_LUA_MUTATION},
    {"armor_status", fun_btarmorstatus, BTECH_LUA_TEXT},
    {"armor_status_ref", fun_btarmorstatus_ref, BTECH_LUA_TEXT},
    {"char_list", fun_btcharlist, BTECH_LUA_LIST},
    {"crit_slot", fun_btcritslot, BTECH_LUA_TEXT},
    {"crit_slot_ref", fun_btcritslot_ref, BTECH_LUA_TEXT},
    {"section_status", fun_btsectstatus, BTECH_LUA_TEXT},
    {"crit_status", fun_btcritstatus, BTECH_LUA_TEXT},
    {"crit_status_ref", fun_btcritstatus_ref, BTECH_LUA_TEXT},
    {"damage_mech", fun_btdamagemech, BTECH_LUA_MUTATION},
    {"damages", fun_btdamages, BTECH_LUA_TEXT},
    {"design_exists", fun_btdesignex, BTECH_LUA_BOOLEAN},
    {"engine_rating", fun_btengrate, BTECH_LUA_NUMBER},
    {"engine_rating_ref", fun_btengrate_ref, BTECH_LUA_NUMBER},
    {"fasa_base_cost_ref", fun_btfasabasecost_ref, BTECH_LUA_NUMBER},
    {"battle_value", fun_btgetbv, BTECH_LUA_NUMBER},
    {"battle_value_ref", fun_btgetbv_ref, BTECH_LUA_NUMBER},
    {"battle_value2_ref", fun_btgetbv2_ref, BTECH_LUA_NUMBER},
    {"defensive_battle_value_ref", fun_btgetdbv_ref, BTECH_LUA_NUMBER},
    {"offensive_battle_value_ref", fun_btgetobv_ref, BTECH_LUA_NUMBER},
    {"char_value", fun_btgetcharvalue, BTECH_LUA_NUMBER},
    {"part_cost", fun_btgetpartcost, BTECH_LUA_NUMBER},
    {"range", fun_btgetrange, BTECH_LUA_NUMBER},
    {"real_max_speed", fun_btgetrealmaxspeed, BTECH_LUA_NUMBER},
    {"get_weight", fun_btgetweight, BTECH_LUA_NUMBER},
    {"xcode_value", fun_btgetxcodevalue, BTECH_LUA_TEXT},
    {"xcode_value_ref", fun_btgetxcodevalue_ref, BTECH_LUA_TEXT},
    {"hex_emit", fun_bthexemit, BTECH_LUA_MUTATION},
    {"hex_in_blast_zone", fun_bthexinblz, BTECH_LUA_BOOLEAN},
    {"hex_line_of_sight", fun_bthexlos, BTECH_LUA_BOOLEAN},
    {"id_to_dbref", fun_btid2db, BTECH_LUA_NUMBER},
    {"lag", fun_btlag, BTECH_LUA_NUMBER},
    {"blast_zones", fun_btlistblz, BTECH_LUA_LIST},
    {"load_map", fun_btloadmap, BTECH_LUA_MUTATION},
    {"load_mech", fun_btloadmech, BTECH_LUA_MUTATION},
    {"mech_line_of_sight", fun_btlosm2m, BTECH_LUA_BOOLEAN},
    {"make_pilot_roll", fun_btmakepilotroll, BTECH_LUA_BOOLEAN},
    {"map_elevation", fun_btmapelev, BTECH_LUA_NUMBER},
    {"map_emit", fun_btmapemit, BTECH_LUA_MUTATION},
    {"map_terrain", fun_btmapterr, BTECH_LUA_TEXT},
    {"map_units", fun_btmapunits, BTECH_LUA_LIST},
    {"mech_frequencies", fun_btmechfreqs, BTECH_LUA_LIST},
    {"repair_job_count", fun_btnumrepjobs, BTECH_LUA_NUMBER},
    {"part_type", fun_btparttype, BTECH_LUA_TEXT},
    {"part_match", fun_btpartmatch, BTECH_LUA_LIST},
    {"part_name", fun_btpartname, BTECH_LUA_TEXT},
    {"part_categories", fun_btpartscategorylist, BTECH_LUA_LIST},
    {"parts", fun_btpartslist, BTECH_LUA_LIST},
    {"part_weight", fun_btgetweight, BTECH_LUA_NUMBER},
    {"payload_ref", fun_btpayload_ref, BTECH_LUA_TEXT},
    {"remove_stores", fun_btremovestores, BTECH_LUA_MUTATION},
    {"set_armor_status", fun_btsetarmorstatus, BTECH_LUA_MUTATION},
    {"set_char_value", fun_btsetcharvalue, BTECH_LUA_MUTATION},
    {"set_max_speed", fun_btsetmaxspeed, BTECH_LUA_MUTATION},
    {"set_part_cost", fun_btsetpartcost, BTECH_LUA_MUTATION},
    {"set_tons", fun_btsettons, BTECH_LUA_MUTATION},
    {"set_xcode_value", fun_btsetxcodevalue, BTECH_LUA_MUTATION},
    {"set_xy", fun_btsetxy, BTECH_LUA_MUTATION},
    {"show_crit_status_ref", fun_btshowcritstatus_ref, BTECH_LUA_TEXT},
    {"show_status_ref", fun_btshowstatus_ref, BTECH_LUA_TEXT},
    {"show_weapon_specs_ref", fun_btshowwspecs_ref, BTECH_LUA_TEXT},
    {"stores", fun_btstores, BTECH_LUA_LIST},
    {"stores_short", fun_btstores_short, BTECH_LUA_LIST},
    {"tech_list", fun_bttechlist, BTECH_LUA_LIST},
    {"tech_list_ref", fun_bttechlist_ref, BTECH_LUA_LIST},
    {"tech_status", fun_bttechstatus, BTECH_LUA_TEXT},
    {"tech_time", fun_bttechtime, BTECH_LUA_NUMBER},
    {"threshold", fun_btthreshold, BTECH_LUA_NUMBER},
    {"tic_weapons", fun_btticweaps, BTECH_LUA_LIST},
    {"under_repair", fun_btunderrepair, BTECH_LUA_BOOLEAN},
    {"unit_fixable", fun_btunitfixable, BTECH_LUA_BOOLEAN},
    {"unit_parts", fun_btunitpartslist, BTECH_LUA_LIST},
    {"unit_parts_ref", fun_btunitpartslist_ref, BTECH_LUA_LIST},
    {"update_links", fun_btupdatelinks, BTECH_LUA_MUTATION},
    {"weapon_status", fun_btweaponstatus, BTECH_LUA_TEXT},
    {"weapon_status_ref", fun_btweaponstatus_ref, BTECH_LUA_TEXT},
    {"weapon_stat", fun_btweapstat, BTECH_LUA_TEXT},
    {"zone_mechs", fun_zmechs, BTECH_LUA_LIST},
    {nullptr, nullptr, BTECH_LUA_TEXT},
};

static void btech_lua_push_list(lua_State *state, char *value) {
  char *cursor = value;
  char *token;
  int index = 1;

  lua_newtable(state);
  while (cursor && *cursor) {
    while (*cursor == ' ' || *cursor == '|')
      cursor++;
    if (!*cursor)
      break;
    token = cursor;
    while (*cursor && *cursor != ' ' && *cursor != '|')
      cursor++;
    if (*cursor)
      *cursor++ = '\0';
    char *end = nullptr;
    long number = strtol(token[0] == '#' ? token + 1 : token, &end, 10);
    if (end && *end == '\0')
      lua_pushinteger(state, number);
    else
      lua_pushstring(state, token);
    lua_rawseti(state, -2, index++);
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
    arguments[index] = alloc_lbuf("btech_lua_argument");
    StringCopy(arguments[index], value);
  }
  entry->handler(buffer, &cursor, GOD, GOD, arguments, argument_count, nullptr,
                 0, &package->services->background_command->evaluation);
  *cursor = '\0';
  for (int index = 0; index < argument_count; index++)
    free_lbuf(arguments[index]);
  if (!strncmp(buffer, "#-", 2) || !strcmp(buffer, "?")) {
    char error[LBUF_SIZE];
    snprintf(error, sizeof(error), "%s", buffer);
    free_lbuf(buffer);
    return luaL_error(state, "%s", error);
  }
  switch (entry->result) {
  case BTECH_LUA_NUMBER:
    lua_pushnumber(state, strtod(buffer, nullptr));
    break;
  case BTECH_LUA_BOOLEAN:
    lua_pushboolean(state, strtol(buffer, nullptr, 10) != 0);
    break;
  case BTECH_LUA_LIST:
    btech_lua_push_list(state, buffer);
    break;
  case BTECH_LUA_MUTATION:
    lua_pushboolean(state, 1);
    break;
  case BTECH_LUA_TEXT:
    lua_pushstring(state, buffer);
    break;
  }
  free_lbuf(buffer);
  return 1;
}

void lua_btech_package_install(lua_State *state, LuaBtechPackage *package) {
  lua_newtable(state);
  for (BtechLuaEntry *entry = btech_lua_entries; entry->name; entry++) {
    lua_pushlightuserdata(state, package);
    lua_pushlightuserdata(state, entry);
    lua_pushcclosure(state, btech_lua_invoke, 2);
    lua_setfield(state, -2, entry->name);
  }
  lua_setglobal(state, "btech");
}
