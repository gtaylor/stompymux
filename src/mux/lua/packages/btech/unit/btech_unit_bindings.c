/* btech_unit_bindings.c - Lua bindings for btech.unit. */

#include <limits.h>
#include <lua.h>
#include <math.h>
#include <stddef.h>
#include <string.h>
#include <strings.h>

#include "btech/combat/mech_damage_api.h"
#include "btech/configuration.h"
#include "btech/special/registry_api.h"
#include "btech/special_objects.h"
#include "btech/unit/mech_build_api.h"
#include "btech/unit/mech_classification_api.h"
#include "btech/unit/mech_consistency_api.h"
#include "btech/unit/mech_equipment_api.h"
#include "btech/unit/mech_internal.h"
#include "btech/unit/mech_partnames_api.h"
#include "btech/unit/mech_radio_api.h"
#include "btech/unit/mech_specification_api.h"
#include "btech/unit/mech_utils_api.h"
#include "btech/unit/template_api.h"
#include "btech/unit/weapon_catalogue_api.h"
#include "equipment_types.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

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
 * @par Lua API definition btech namespace btech.unit
 * @code{.lua}
 * ---Live units, templates, combat values, and status.
 * ---@class BtechUnitPackage
 * local btech_unit = {}
 * @endcode
 */
static DbRef require_unit(lua_State *state, LuaBtechPackage *package,
                          int argument) {
  return lua_btech_require_special(package, state, argument, BTECH_SPECIAL_MECH,
                                   "unit");
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

static Mech *require_mech(lua_State *state, LuaBtechPackage *package,
                          int argument) {
  const DbRef UNIT = require_unit(state, package, argument);
  Mech *mech = btech_context_get_mech(lua_btech_context(package), UNIT);
  if (mech == nullptr)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                        "unit runtime state is unavailable");
  return mech;
}

int lua_btech_optional_section(lua_State *state, Mech *mech, int argument) {
  if (lua_isnoneornil(state, argument))
    return -1;
  if (lua_type(state, argument) != LUA_TSTRING)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "section must be a string");
  const char *wanted = lua_tostring(state, argument);
  const UnitSectionCatalog CATALOG = {
      .unit_type = mech_class(mech), .movement_type = mech_movement_type(mech)};
  for (size_t index = 0; index < unit_section_name_count(&CATALOG); index++) {
    const char *name = unit_section_name(&CATALOG, index);
    const ArmorSectionAbbreviation ABBREVIATION = armor_section_abbreviation(
        &(ArmorSectionReference){.unit_class = mech_class(mech),
                                 .movement_type = mech_movement_type(mech),
                                 .location = (int)index});
    if (strcasecmp(wanted, name) == 0 ||
        strcasecmp(wanted, ABBREVIATION.text) == 0)
      return (int)index;
  }
  (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                      "unknown section");
  return -1;
}

static void push_armor_pair(lua_State *state, int current, int original,
                            const char *field) {
  lua_newtable(state);
  lua_pushinteger(state, current);
  lua_setfield(state, -2, "current");
  lua_pushinteger(state, original);
  lua_setfield(state, -2, "original");
  lua_setfield(state, -2, field);
}

void lua_btech_push_armor(lua_State *state, Mech *mech, int section) {
  const UnitSectionCatalog CATALOG = {
      .unit_type = mech_class(mech), .movement_type = mech_movement_type(mech)};
  int armor = 0;
  int original_armor = 0;
  int internal = 0;
  int original_internal = 0;
  int rear = 0;
  int original_rear = 0;
  const int START = section < 0 ? 0 : section;
  const int END =
      section < 0 ? (int)unit_section_name_count(&CATALOG) : section + 1;
  for (int index = START; index < END; index++) {
    armor += mech_section_armor(mech, index);
    original_armor += mech_section_original_armor(mech, index);
    internal += mech_section_internal(mech, index);
    original_internal += mech_section_original_internal(mech, index);
    rear += mech_section_rear_armor(mech, index);
    original_rear += mech_section_original_rear_armor(mech, index);
  }
  lua_newtable(state);
  if (section >= 0) {
    lua_pushstring(state, unit_section_name(&CATALOG, (size_t)section));
    lua_setfield(state, -2, "section");
  }
  push_armor_pair(state, armor, original_armor, "armor");
  push_armor_pair(state, internal, original_internal, "internal");
  push_armor_pair(state, rear, original_rear, "rear_armor");
}

static int lua_btech_unit_armor(lua_State *state, LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package, 1);
  lua_btech_push_armor(state, mech, lua_btech_optional_section(state, mech, 2));
  return 1;
}

static const char *critical_kind(int part) {
  if (part == EMPTY)
    return "empty";
  if (equipment_is_weapon(part))
    return "weapon";
  if (equipment_is_ammunition(part))
    return "ammunition";
  if (equipment_is_bomb(part))
    return "bomb";
  if (equipment_is_special(part))
    return "special";
  if (equipment_is_cargo(part))
    return "cargo";
  return "other";
}

void lua_btech_push_critical_slots(lua_State *state, BtechContext *context,
                                   Mech *mech, int section) {
  const UnitSectionCatalog CATALOG = {
      .unit_type = mech_class(mech), .movement_type = mech_movement_type(mech)};
  lua_newtable(state);
  for (int slot = 0; slot < NUM_CRITICALS; slot++) {
    const int PART = mech_critical_part_type(mech, section, slot);
    const int BRAND = mech_critical_brand(mech, section, slot);
    lua_newtable(state);
    lua_pushstring(state, unit_section_name(&CATALOG, (size_t)section));
    lua_setfield(state, -2, "section");
    lua_pushinteger(state, slot + 1);
    lua_setfield(state, -2, "slot");
    lua_pushstring(state, critical_kind(PART));
    lua_setfield(state, -2, "kind");
    if (PART != EMPTY) {
      lua_btech_push_part(state, context,
                          (PartReference){.id = PART, .brand = BRAND});
      lua_setfield(state, -2, "part");
    }
    lua_pushboolean(
        state, mech_critical_is_nonfunctional(mech, section, slot) ? 0 : 1);
    lua_setfield(state, -2, "operational");
    lua_pushboolean(state,
                    mech_critical_temporary_failure(mech, section, slot) != 0);
    lua_setfield(state, -2, "temporary_failure");
    lua_pushinteger(state, mech_critical_data(mech, section, slot));
    lua_setfield(state, -2, "auxiliary_data");
    if (equipment_is_ammunition(PART)) {
      lua_newtable(state);
      lua_pushinteger(state, mech_critical_data(mech, section, slot));
      lua_setfield(state, -2, "rounds");
      lua_pushinteger(state,
                      mech_critical_full_ammunition(mech, section, slot));
      lua_setfield(state, -2, "capacity");
      lua_setfield(state, -2, "ammunition");
    }
    lua_btech_push_critical_modes(
        state, (unsigned int)mech_critical_fire_mode(mech, section, slot),
        false);
    lua_setfield(state, -2, "fire_modes");
    lua_btech_push_critical_modes(
        state, (unsigned int)mech_critical_ammo_mode(mech, section, slot),
        true);
    lua_setfield(state, -2, "ammunition_modes");
    lua_rawseti(state, -2, slot + 1);
  }
}

static int lua_btech_unit_critical_slots(lua_State *state,
                                         LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package, 1);
  const int SECTION = lua_btech_optional_section(state, mech, 2);
  if (SECTION < 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "section is required");
  lua_btech_push_critical_slots(state, lua_btech_context(package), mech,
                                SECTION);
  return 1;
}

void lua_btech_push_weapons(lua_State *state, BtechContext *context, Mech *mech,
                            int selected_section) {
  const UnitSectionCatalog CATALOG = {
      .unit_type = mech_class(mech), .movement_type = mech_movement_type(mech)};
  unsigned char weapons[MAX_WEAPS_SECTION];
  unsigned char recycle[MAX_WEAPS_SECTION];
  int criticals[MAX_WEAPS_SECTION];
  int game_number = 0;
  int output = 1;
  lua_newtable(state);
  for (int section = 0; section < (int)unit_section_name_count(&CATALOG);
       section++) {
    const int COUNT =
        find_weapons_advanced(mech, section, weapons, recycle, criticals, 1);
    for (int index = 0; index < COUNT; index++, game_number++) {
      if (selected_section >= 0 && section != selected_section)
        continue;
      const int WEAPON = *(const unsigned char *)checked_storage_at_const(
          weapons, MAX_WEAPS_SECTION, sizeof(weapons[0]), (size_t)index);
      const int SLOT = *(const int *)checked_storage_at_const(
          criticals, MAX_WEAPS_SECTION, sizeof(criticals[0]), (size_t)index);
      const int CURRENT_RECYCLE =
          *(const unsigned char *)checked_storage_at_const(
              recycle, MAX_WEAPS_SECTION, sizeof(recycle[0]), (size_t)index);
      const int PART = WEAPON + WEAPON_BASE_INDEX;
      lua_newtable(state);
      lua_pushinteger(state, game_number);
      lua_setfield(state, -2, "number");
      lua_pushstring(state, unit_section_name(&CATALOG, (size_t)section));
      lua_setfield(state, -2, "section");
      lua_pushinteger(state, SLOT + 1);
      lua_setfield(state, -2, "first_slot");
      lua_btech_push_part(
          state, context,
          (PartReference){.id = PART,
                          .brand = mech_critical_brand(mech, section, SLOT)});
      lua_setfield(state, -2, "part");
      lua_pushinteger(state, get_weapon_crits(mech, WEAPON));
      lua_setfield(state, -2, "slot_count");
      lua_pushinteger(state, CURRENT_RECYCLE);
      lua_setfield(state, -2, "recycle");
      lua_pushinteger(state, weapon_catalogue_recycle_time(WEAPON));
      lua_setfield(state, -2, "recycle_time");
      lua_pushboolean(state,
                      weapon_is_nonfunctional(mech, section, SLOT,
                                              get_weapon_crits(mech, WEAPON))
                          ? 0
                          : 1);
      lua_setfield(state, -2, "operational");
      lua_rawseti(state, -2, output++);
    }
  }
}

static int lua_btech_unit_weapons(lua_State *state, LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package, 1);
  lua_btech_push_weapons(state, lua_btech_context(package), mech,
                         lua_btech_optional_section(state, mech, 2));
  return 1;
}

static void push_radio_modes(lua_State *state, int modes) {
  static const struct {
    int flag;
    const char *name;
  } MODES[] = {{FREQ_DIGITAL, "digital"},
               {FREQ_MUTE, "mute"},
               {FREQ_RELAY, "relay"},
               {FREQ_INFO, "information"},
               {FREQ_SCAN, "scan"}};
  int output = 1;
  lua_newtable(state);
  for (size_t index = 0; index < sizeof(MODES) / sizeof(MODES[0]); index++) {
    const typeof(MODES[0]) *mode = checked_storage_at_const(
        MODES, sizeof(MODES) / sizeof(MODES[0]), sizeof(MODES[0]), index);
    if ((modes & mode->flag) == 0)
      continue;
    lua_pushstring(state, mode->name);
    lua_rawseti(state, -2, output++);
  }
}

static int lua_btech_unit_radio_channels(lua_State *state,
                                         LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package, 1);
  lua_newtable(state);
  for (int channel = 0; channel < mech_radio_channel_count(mech); channel++) {
    lua_newtable(state);
    lua_pushinteger(state, channel + 1);
    lua_setfield(state, -2, "channel");
    lua_pushinteger(state, mech_radio_frequency(mech, channel));
    lua_setfield(state, -2, "frequency");
    lua_pushstring(state, mech_radio_title(mech, channel));
    lua_setfield(state, -2, "title");
    push_radio_modes(state, mech_radio_mode(mech, channel));
    lua_setfield(state, -2, "modes");
    lua_rawseti(state, -2, channel + 1);
  }
  return 1;
}

static int lua_btech_unit_engine(lua_State *state, LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package, 1);
  lua_newtable(state);
  lua_pushinteger(state, mech_engine_rating(mech));
  lua_setfield(state, -2, "rating");
  lua_pushinteger(state, susp_factor(mech));
  lua_setfield(state, -2, "suspension_factor");
  return 1;
}

static bool optional_skill(lua_State *state, int table, const char *field,
                           int *value) {
  lua_btech_get_field(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  if (lua_type(state, -1) != LUA_TNUMBER) {
    lua_pop(state, 1);
    (void)lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be an integer", field);
  }
  const lua_Number NUMBER = lua_tonumber(state, -1);
  lua_pop(state, 1);
  if (!isfinite(NUMBER) || floor(NUMBER) != NUMBER || NUMBER < 0 || NUMBER > 20)
    (void)lua_error_arg(state, table, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be an integer from 0 through 20", field);
  *value = (int)NUMBER;
  return true;
}

void lua_btech_push_battle_value(lua_State *state, Mech *mech, int options,
                                 bool template_defaults) {
  static const char *const FIELDS[] = {"rules", "gunnery", "piloting"};
  const char *rules = "bv2";
  int gunnery = template_defaults ? 4 : find_average_gunnery(mech);
  int piloting = template_defaults ? 5 : find_pilot_piloting(mech);
  if (!lua_isnoneornil(state, options)) {
    lua_btech_check_options(state, options, FIELDS,
                            sizeof(FIELDS) / sizeof(FIELDS[0]), options);
    lua_btech_get_field(state, options, "rules");
    if (!lua_isnil(state, -1)) {
      if (lua_type(state, -1) != LUA_TSTRING)
        (void)lua_error_arg(state, options, LUA_ERROR_CODE_ARG_INVALID,
                            "rules must be 'bv2' or 'legacy'");
      rules = lua_tostring(state, -1);
    }
    lua_pop(state, 1);
  }
  lua_newtable(state);
  lua_pushstring(state, rules);
  lua_setfield(state, -2, "rules");
  if (strcmp(rules, "bv2") == 0) {
    if (!lua_isnoneornil(state, options)) {
      lua_btech_get_field(state, options, "gunnery");
      const bool GUNNERY = lua_isnil(state, -1) == 0;
      lua_pop(state, 1);
      lua_btech_get_field(state, options, "piloting");
      const bool PILOTING = lua_isnil(state, -1) == 0;
      lua_pop(state, 1);
      if (GUNNERY || PILOTING)
        (void)lua_error_arg(state, options, LUA_ERROR_CODE_ARG_INVALID,
                            "skill overrides are invalid for BV2");
    }
    const float OFFENSIVE_FLOAT = calculate_offensive_bv(mech);
    const float DEFENSIVE_FLOAT = calculate_defensive_bv(mech);
    const lua_Number OFFENSIVE = (lua_Number)OFFENSIVE_FLOAT;
    const lua_Number DEFENSIVE = (lua_Number)DEFENSIVE_FLOAT;
    lua_pushnumber(state, OFFENSIVE + DEFENSIVE);
    lua_setfield(state, -2, "total");
    lua_pushnumber(state, OFFENSIVE);
    lua_setfield(state, -2, "offensive");
    lua_pushnumber(state, DEFENSIVE);
    lua_setfield(state, -2, "defensive");
    return;
  }
  if (strcmp(rules, "legacy") != 0)
    (void)lua_error_arg(state, options, LUA_ERROR_CODE_ARG_INVALID,
                        "rules must be 'bv2' or 'legacy'");
  if (!lua_isnoneornil(state, options)) {
    const bool HAS_GUNNERY =
        optional_skill(state, options, "gunnery", &gunnery);
    const bool HAS_PILOTING =
        optional_skill(state, options, "piloting", &piloting);
    if (HAS_GUNNERY != HAS_PILOTING)
      (void)lua_error_arg(state, options, LUA_ERROR_CODE_ARG_INVALID,
                          "gunnery and piloting must be supplied together");
  }
  lua_pushinteger(state, calculate_bv(mech, gunnery, piloting));
  lua_setfield(state, -2, "total");
  lua_pushinteger(state, gunnery);
  lua_setfield(state, -2, "gunnery");
  lua_pushinteger(state, piloting);
  lua_setfield(state, -2, "piloting");
}

static int lua_btech_unit_battle_value(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_push_battle_value(state, require_mech(state, package, 1), 2, false);
  return 1;
}

static const char *optional_string_field(lua_State *state, int table,
                                         const char *field, int argument) {
  lua_btech_get_field(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return nullptr;
  }
  if (lua_type(state, -1) != LUA_TSTRING)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a string", field);
  const char *value = lua_tostring(state, -1);
  lua_pop(state, 1);
  return value;
}

static bool optional_boolean_field(lua_State *state, int table,
                                   const char *field, int argument) {
  lua_btech_get_field(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  if (!lua_isboolean(state, -1))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a boolean", field);
  const bool VALUE = lua_toboolean(state, -1) != 0;
  lua_pop(state, 1);
  return VALUE;
}

static int lua_btech_unit_apply_damage(lua_State *state,
                                       LuaBtechPackage *package) {
  static const char *const FIELDS[] = {
      "amount",         "cluster_size", "direction_code",
      "force_critical", "unit_message", "map_message",
  };
  Mech *mech = require_mech(state, package, 1);
  lua_btech_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          2);
  const int AMOUNT =
      (int)lua_btech_check_integer_field(state, 2, "amount", 1, 1000, 2);
  const int CLUSTER =
      (int)lua_btech_check_integer_field(state, 2, "cluster_size", 1, 1000, 2);
  const int DIRECTION =
      (int)lua_btech_check_integer_field(state, 2, "direction_code", 0, 21, 2);
  if (!mech_damage_apply_clusters(&(DamageClusterRequest){
          .mech = mech,
          .total_damage = AMOUNT,
          .cluster_size = CLUSTER,
          .direction = DIRECTION,
          .critical = optional_boolean_field(state, 2, "force_critical", 2),
          .mech_message = optional_string_field(state, 2, "unit_message", 2),
          .broadcast_message =
              optional_string_field(state, 2, "map_message", 2)}))
    return lua_btech_operation_error(state, "damage_failed",
                                     "damage operation failed");
  return 0;
}

static void push_inventory(lua_State *state, BtechContext *context, Mech *mech,
                           bool payload_only) {
  int quantities[(BRANDCOUNT + 1) * NUM_ITEMS] = {0};
  for (int section = 0; section < NUM_SECTIONS; section++) {
    int previous_part = EMPTY;
    int previous_brand = 0;
    for (int slot = 0; slot < NUM_CRITICALS; slot++) {
      const int PART = mech_critical_part_type(mech, section, slot);
      const int BRAND = mech_critical_brand(mech, section, slot);
      const bool INCLUDED =
          (bool)(PART != EMPTY && (!payload_only || equipment_is_weapon(PART) ||
                                   equipment_is_ammunition(PART)));
      if (!INCLUDED || mech_critical_is_destroyed(mech, section, slot)) {
        previous_part = EMPTY;
        continue;
      }
      if (equipment_is_weapon(PART) && PART == previous_part &&
          BRAND == previous_brand)
        continue;
      const size_t INDEX = ((size_t)BRAND * NUM_ITEMS) + (size_t)PART;
      if (BRAND >= 0 && BRAND <= BRANDCOUNT && PART >= 0 && PART < NUM_ITEMS)
        (*(int *)checked_storage_at(
            quantities, ((size_t)BRANDCOUNT + 1U) * (size_t)NUM_ITEMS,
            sizeof(quantities[0]), INDEX))++;
      previous_part = PART;
      previous_brand = BRAND;
    }
  }
  int output = 1;
  lua_newtable(state);
  for (size_t index = 0; index < part_name_count(context); index++) {
    const PartReference PART =
        part_name_reference(part_name_at(context, index));
    if (PART.brand < 0 || PART.brand > BRANDCOUNT || PART.id < 0 ||
        PART.id >= NUM_ITEMS)
      (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                            "part catalogue contains an invalid identity");
    const size_t INDEX = ((size_t)PART.brand * NUM_ITEMS) + (size_t)PART.id;
    const int QUANTITY = *(const int *)checked_storage_at_const(
        quantities, ((size_t)BRANDCOUNT + 1U) * (size_t)NUM_ITEMS,
        sizeof(quantities[0]), INDEX);
    if (QUANTITY == 0)
      continue;
    lua_newtable(state);
    lua_btech_push_part(state, context, PART);
    lua_setfield(state, -2, "part");
    lua_pushinteger(state, QUANTITY);
    lua_setfield(state, -2, "quantity");
    lua_rawseti(state, -2, output++);
  }
}

void lua_btech_push_payload(lua_State *state, BtechContext *context,
                            Mech *mech) {
  push_inventory(state, context, mech, true);
}

void lua_btech_push_installed_parts(lua_State *state, BtechContext *context,
                                    Mech *mech) {
  push_inventory(state, context, mech, false);
}

static int lua_btech_unit_payload(lua_State *state, LuaBtechPackage *package) {
  lua_btech_push_payload(state, lua_btech_context(package),
                         require_mech(state, package, 1));
  return 1;
}

static int lua_btech_unit_installed_parts(lua_State *state,
                                          LuaBtechPackage *package) {
  lua_btech_push_installed_parts(state, lua_btech_context(package),
                                 require_mech(state, package, 1));
  return 1;
}

static void push_technology_group(lua_State *state, const char *group,
                                  size_t count, const char *(*name_at)(size_t),
                                  unsigned configured, unsigned inferred,
                                  int *output) {
  const size_t BIT_COUNT = sizeof(unsigned) * CHAR_BIT;
  for (size_t index = 0; index < count && index < BIT_COUNT; index++) {
    const unsigned BIT = 1U << index;
    if (((configured | inferred) & BIT) == 0)
      continue;
    const char *name = name_at(index);
    lua_newtable(state);
    lua_pushstring(state, name);
    lua_setfield(state, -2, "code");
    lua_pushstring(state, name);
    lua_setfield(state, -2, "name");
    lua_pushstring(state, group);
    lua_setfield(state, -2, "group");
    lua_pushstring(state, (configured & BIT) != 0 ? "configured" : "inferred");
    lua_setfield(state, -2, "source");
    lua_rawseti(state, -2, (*output)++);
  }
}

void lua_btech_push_technologies(lua_State *state, Mech *mech) {
  const unsigned PRIMARY = (unsigned)mech_technology_flags(mech);
  const unsigned SECONDARY = (unsigned)mech_technology_flags_secondary(mech);
  const unsigned INFANTRY = (unsigned)mech_infantry_technology_flags(mech);
  Mech inferred = *mech;
  update_specials(&inferred);
  int output = 1;
  lua_newtable(state);
  push_technology_group(state, "primary", primary_technology_name_count(),
                        primary_technology_name, PRIMARY,
                        (unsigned)mech_technology_flags(&inferred) & ~PRIMARY,
                        &output);
  push_technology_group(state, "secondary", secondary_technology_name_count(),
                        secondary_technology_name, SECONDARY,
                        (unsigned)mech_technology_flags_secondary(&inferred) &
                            ~SECONDARY,
                        &output);
  push_technology_group(
      state, "infantry", infantry_technology_name_count(),
      infantry_technology_name, INFANTRY,
      (unsigned)mech_infantry_technology_flags(&inferred) & ~INFANTRY, &output);
}

static int lua_btech_unit_technologies(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_push_technologies(state, require_mech(state, package, 1));
  return 1;
}

/**
 * @par Lua API definition btech callable btech.unit.preferred_id
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
 * @par Lua API definition btech callable btech.unit.set_preferred_id
 * @code{.lua}
 * ---Sets a unit's preferred map identifier, or clears it with nil.
 * ---@param unit DbRef|Object
 * ---@param preferred_id string|nil
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
  return 0;
}

/**
 * @par Lua API definition btech callable btech.unit.markings
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

static int lua_btech_unit_display_name(lua_State *state,
                                       LuaBtechPackage *package) {
  const DbRef UNIT = require_unit(state, package, 1);
  const char *value = btech_unit_display_name(lua_btech_context(package), UNIT);
  if (*value == '\0')
    lua_pushnil(state);
  else
    lua_pushstring(state, value);
  return 1;
}

static int lua_btech_unit_set_display_name(lua_State *state,
                                           LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef UNIT = require_unit(state, package, 1);
  const char *value = optional_string(state, 2, LBUF_SIZE - 1, "name");
  if (!btech_unit_display_name_set(lua_btech_context(package), UNIT, value))
    return lua_btech_operation_error(state, "display_name_store_failed",
                                     "unable to store unit display name");
  return 0;
}

/**
 * @par Lua API definition btech callable btech.unit.set_markings
 * @code{.lua}
 * ---Sets a unit's markings, or clears them with nil.
 * ---@param unit DbRef|Object
 * ---@param markings string|nil
 * function btech_unit.set_markings(unit, markings) end
 * @endcode
 */
static int lua_btech_unit_set_markings(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef UNIT = require_unit(state, package, 1);
  const char *value = optional_string(state, 2, LBUF_SIZE - 1, "markings");
  if (!btech_unit_markings_set(lua_btech_context(package), UNIT, value))
    return lua_btech_operation_error(state, "markings_store_failed",
                                     "unable to store unit markings");
  return 0;
}

/**
 * @par Lua API definition btech callable btech.unit.assigned_pilot
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
  lua_btech_push_optional_object(state, package, PILOT);
  return 1;
}

/**
 * @par Lua API definition btech callable btech.unit.set_assigned_pilot
 * @code{.lua}
 * ---Sets a unit's assigned pilot, or clears the assignment with nil.
 * ---@param unit DbRef|Object
 * ---@param pilot DbRef|Object|nil
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
    return lua_error_arg(state, 2, LUA_ERROR_CODE_OBJECT_INVALID,
                         "assigned pilot must be a player");
  if (!btech_unit_assigned_pilot_set(lua_btech_context(package), UNIT, PILOT))
    return lua_btech_operation_error(state, "assigned_pilot_store_failed",
                                     "unable to store assigned pilot");
  return 0;
}

static const BtechLuaNativeEntry BTECH_UNIT_NATIVE_ENTRIES[] = {
    {"armor", "unit.armor", lua_btech_unit_armor},
    {"critical_slots", "unit.critical_slots", lua_btech_unit_critical_slots},
    {"weapons", "unit.weapons", lua_btech_unit_weapons},
    {"radio_channels", "unit.radio_channels", lua_btech_unit_radio_channels},
    {"engine", "unit.engine", lua_btech_unit_engine},
    {"battle_value", "unit.battle_value", lua_btech_unit_battle_value},
    {"apply_damage", "unit.apply_damage", lua_btech_unit_apply_damage},
    {"payload", "unit.payload", lua_btech_unit_payload},
    {"installed_parts", "unit.installed_parts", lua_btech_unit_installed_parts},
    {"technologies", "unit.technologies", lua_btech_unit_technologies},
    {"preferred_id", "unit.preferred_id", lua_btech_unit_preferred_id},
    {"set_preferred_id", "unit.set_preferred_id",
     lua_btech_unit_set_preferred_id},
    {"markings", "unit.markings", lua_btech_unit_markings},
    {"set_markings", "unit.set_markings", lua_btech_unit_set_markings},
    {"display_name", "unit.display_name", lua_btech_unit_display_name},
    {"set_display_name", "unit.set_display_name",
     lua_btech_unit_set_display_name},
    {"assigned_pilot", "unit.assigned_pilot", lua_btech_unit_assigned_pilot},
    {"set_assigned_pilot", "unit.set_assigned_pilot",
     lua_btech_unit_set_assigned_pilot},
};

void lua_btech_install_unit_bindings(lua_State *state,
                                     LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "unit", BTECH_UNIT_NATIVE_ENTRIES,
      sizeof(BTECH_UNIT_NATIVE_ENTRIES) / sizeof(BTECH_UNIT_NATIVE_ENTRIES[0]));
  lua_btech_install_unit_operation_bindings(state, package);
}
