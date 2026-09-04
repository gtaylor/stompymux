/* btech_unit_bindings.c - Lua bindings for btech.unit. */

#include <lua.h>
#include <string.h>

#include "btech/configuration.h"
#include "btech/scripting/script_functions_api.h"
#include "btech/special_objects.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"

/**
 * @par LuaLS definition btech alias btech.unit.critical-slot-field
 * @code{.lua}
 * ---@alias CriticalSlotField "NAME"|"STATUS"|"DATA"|"MAXAMMO"|"AMMOTYPE"|"MODE"|"HALFTON" Canonical critical-slot field; native matching is ASCII-case-insensitive.
 * @endcode
 *
 * @par LuaLS definition btech alias btech.unit.armor-status-field
 * @code{.lua}
 * ---@alias ArmorStatusField 0|1|2 Armor field: current armor, internal structure, or rear armor.
 * @endcode
 *
 * @par LuaLS definition btech namespace btech.unit
 * @code{.lua}
 * ---Live units, templates, combat values, and status.
 * ---@class BtechUnitPackage
 * local btech_unit = {}
 * @endcode
 */
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
    {"display_name", "unit.display_name", fun_btunitdisplayname},
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
    {"set_display_name", "unit.set_display_name", fun_btsetunitdisplayname},
    {"set_max_speed", "unit.set_max_speed", fun_btsetmaxspeed},
    {"set_tons", "unit.set_tons", fun_btsettons},
    {"set_value", "unit.set_value", fun_btsetunitvalue},
    {"show_crit_status_ref", "unit.show_crit_status_ref",
     fun_btshowcritstatus_ref},
    {"show_status_ref", "unit.show_status_ref", fun_btshowstatus_ref},
    {"show_weapon_specs_ref", "unit.show_weapon_specs_ref",
     fun_btshowwspecs_ref},
    {"tic_weapons", "unit.tic_weapons", fun_btticweaps},
    {"value", "unit.value", fun_btgetunitvalue},
    {"value_ref", "unit.value_ref", fun_btgetunitvalue_ref},
    {"weapon_status", "unit.weapon_status", fun_btweaponstatus},
    {"weapon_status_ref", "unit.weapon_status_ref", fun_btweaponstatus_ref},
};

static DbRef require_unit(lua_State *state, LuaBtechPackage *package,
                          int argument) {
  const DbRef UNIT = lua_btech_require_object(package, state, argument);
  if (btech_special_object_type(lua_btech_context(package), UNIT) !=
      BTECH_SPECIAL_MECH)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_BTECH_FAILED,
                        "object is not a registered BTech unit");
  return UNIT;
}

static const char *optional_string(lua_State *state, int argument,
                                   size_t maximum, const char *label) {
  if (lua_isnil(state, argument))
    return nullptr;
  if (lua_type(state, argument) != LUA_TSTRING)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a string or nil", label);
  size_t length;
  const char *value = lua_tolstring(state, argument, &length);
  if (length == 0 || length > maximum)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must contain 1 to %zu bytes", label, maximum);
  return value;
}

/**
 * @par LuaLS definition btech callable btech.unit.preferred_id
 * @code{.lua}
 * ---Returns a unit's preferred two-letter map identifier, or nil.
 * ---@param unit DbRef|Object
 * ---@return string|nil preferred_id
 * function btech_unit.preferred_id(unit) end
 * @endcode
 */
static int lua_btech_unit_preferred_id(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef UNIT = require_unit(state, package, 1);
  const char *value = btech_unit_preferred_id(lua_btech_context(package), UNIT);
  if (*value == '\0')
    lua_pushnil(state);
  else
    lua_pushstring(state, value);
  return 1;
}

/**
 * @par LuaLS definition btech callable btech.unit.set_preferred_id
 * @code{.lua}
 * ---Sets a unit's preferred map identifier, or clears it with nil.
 * ---@param unit DbRef|Object
 * ---@param preferred_id string|nil
 * ---@return true success
 * function btech_unit.set_preferred_id(unit, preferred_id) end
 * @endcode
 */
static int lua_btech_unit_set_preferred_id(lua_State *state,
                                           LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef UNIT = require_unit(state, package, 1);
  const char *value = optional_string(state, 2, 2, "preferred_id");
  if (!btech_unit_preferred_id_set(lua_btech_context(package), UNIT, value))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "preferred_id must contain exactly two ASCII letters");
  lua_pushboolean(state, 1);
  return 1;
}

/**
 * @par LuaLS definition btech callable btech.unit.markings
 * @code{.lua}
 * ---Returns a unit's markings, or nil.
 * ---@param unit DbRef|Object
 * ---@return string|nil markings
 * function btech_unit.markings(unit) end
 * @endcode
 */
static int lua_btech_unit_markings(lua_State *state, LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef UNIT = require_unit(state, package, 1);
  const char *value = btech_unit_markings(lua_btech_context(package), UNIT);
  if (*value == '\0')
    lua_pushnil(state);
  else
    lua_pushstring(state, value);
  return 1;
}

/**
 * @par LuaLS definition btech callable btech.unit.set_markings
 * @code{.lua}
 * ---Sets a unit's markings, or clears them with nil.
 * ---@param unit DbRef|Object
 * ---@param markings string|nil
 * ---@return true success
 * function btech_unit.set_markings(unit, markings) end
 * @endcode
 */
static int lua_btech_unit_set_markings(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef UNIT = require_unit(state, package, 1);
  const char *value = optional_string(state, 2, LBUF_SIZE - 1, "markings");
  if (!btech_unit_markings_set(lua_btech_context(package), UNIT, value))
    return lua_error_raise(state, LUA_ERROR_CODE_BTECH_FAILED,
                           "unable to store unit markings");
  lua_pushboolean(state, 1);
  return 1;
}

/**
 * @par LuaLS definition btech callable btech.unit.assigned_pilot
 * @code{.lua}
 * ---Returns a unit's assigned pilot as an Object, or nil.
 * ---@param unit DbRef|Object
 * ---@return Object|nil pilot
 * function btech_unit.assigned_pilot(unit) end
 * @endcode
 */
static int lua_btech_unit_assigned_pilot(lua_State *state,
                                         LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef UNIT = require_unit(state, package, 1);
  const DbRef PILOT =
      btech_unit_assigned_pilot(lua_btech_context(package), UNIT);
  if (PILOT == NOTHING)
    lua_pushnil(state);
  else
    lua_btech_push_object(state, package, PILOT);
  return 1;
}

/**
 * @par LuaLS definition btech callable btech.unit.set_assigned_pilot
 * @code{.lua}
 * ---Sets a unit's assigned pilot, or clears the assignment with nil.
 * ---@param unit DbRef|Object
 * ---@param pilot DbRef|Object|nil
 * ---@return true success
 * function btech_unit.set_assigned_pilot(unit, pilot) end
 * @endcode
 */
static int lua_btech_unit_set_assigned_pilot(lua_State *state,
                                             LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef UNIT = require_unit(state, package, 1);
  const DbRef PILOT = lua_isnil(state, 2)
                          ? NOTHING
                          : lua_btech_require_object(package, state, 2);
  if (PILOT != NOTHING && !is_player(package->services->database, PILOT))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_FAILED,
                         "assigned pilot must be a player");
  if (!btech_unit_assigned_pilot_set(lua_btech_context(package), UNIT, PILOT))
    return lua_error_raise(state, LUA_ERROR_CODE_BTECH_FAILED,
                           "unable to store assigned pilot");
  lua_pushboolean(state, 1);
  return 1;
}

static const BtechLuaNativeEntry BTECH_UNIT_NATIVE_ENTRIES[] = {
    {"preferred_id", "unit.preferred_id", lua_btech_unit_preferred_id},
    {"set_preferred_id", "unit.set_preferred_id",
     lua_btech_unit_set_preferred_id},
    {"markings", "unit.markings", lua_btech_unit_markings},
    {"set_markings", "unit.set_markings", lua_btech_unit_set_markings},
    {"assigned_pilot", "unit.assigned_pilot", lua_btech_unit_assigned_pilot},
    {"set_assigned_pilot", "unit.set_assigned_pilot",
     lua_btech_unit_set_assigned_pilot},
};

void lua_btech_install_unit_bindings(lua_State *state,
                                     LuaBtechPackage *package) {
  lua_btech_install_bindings(state, package, "unit", BTECH_UNIT_ENTRIES,
                             sizeof(BTECH_UNIT_ENTRIES) /
                                 sizeof(BTECH_UNIT_ENTRIES[0]));
  lua_btech_install_native_bindings(
      state, package, "unit", BTECH_UNIT_NATIVE_ENTRIES,
      sizeof(BTECH_UNIT_NATIVE_ENTRIES) / sizeof(BTECH_UNIT_NATIVE_ENTRIES[0]));
}
