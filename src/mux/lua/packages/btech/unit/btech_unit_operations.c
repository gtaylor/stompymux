/* btech_unit_operations.c - Live-unit Lua actions and state queries. */

#include <limits.h>
#include <lua.h>
#include <math.h>
#include <stddef.h>

#include "btech/combat/crit_api.h"
#include "btech/commands/mech_restrict_api.h"
#include "btech/configuration.h"
#include "btech/context.h"
#include "btech/movement/mech_move_api.h"
#include "btech/special/registry_api.h"
#include "btech/special_objects.h"
#include "btech/unit/mech_build_api.h"
#include "btech/unit/mech_classification_api.h"
#include "btech/unit/mech_equipment_api.h"
#include "btech/unit/mech_partnames_api.h"
#include "btech/unit/mech_specification_api.h"
#include "btech/unit/mech_template_api.h"
#include "btech/unit/mech_tic_api.h"
#include "btech/unit/mech_utils_api.h"
#include "btech/unit/template_api.h"
#include "btech/unit/weapon_catalogue_api.h"
#include "equipment_types.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static Mech *require_mech(lua_State *state, LuaBtechPackage *package) {
  const DbRef UNIT =
      lua_btech_require_special(package, state, 1, BTECH_SPECIAL_MECH, "unit");
  Mech *mech = btech_context_get_mech(lua_btech_context(package), UNIT);
  if (mech == nullptr)
    (void)lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                        "unit runtime state is unavailable");
  return mech;
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters): The bounds form a natural
// ordered pair and are named at every call site.
static lua_Number require_number(lua_State *state, int argument,
                                 lua_Number minimum, lua_Number maximum,
                                 const char *label) {
  if (lua_type(state, argument) != LUA_TNUMBER)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a number", label);
  const lua_Number VALUE = lua_tonumber(state, argument);
  if (!isfinite(VALUE) || VALUE < minimum || VALUE > maximum)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "%s is outside its valid range", label);
  return VALUE;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

static int lua_btech_unit_load_template(lua_State *state,
                                        LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package);
  if (lua_type(state, 2) != LUA_TSTRING || lua_objlen(state, 2) == 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "reference must be a non-empty string");
  const char *reference = lua_tostring(state, 2);
  lua_btech_validate_resource_name(state, 2, reference, "reference");
  BtechContext *context = lua_btech_context(package);
  if (mech_template_resolve_path(context,
                                 btech_context_mech_template_path(context),
                                 reference) == nullptr)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_TEMPLATE_NOT_FOUND,
                         "template was not found");
  if (!mech_template_load(GOD, mech, reference))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_TEMPLATE_INVALID,
                         "template is malformed");
  clear_mech_from_los(mech);
  return 0;
}

static int lua_btech_unit_piloting_check(lua_State *state,
                                         LuaBtechPackage *package) {
  static const char *const FIELDS[] = {"roll_modifier", "damage_modifier"};
  Mech *mech = require_mech(state, package);
  lua_btech_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          2);
  const int ROLL = (int)lua_btech_check_integer_field(state, 2, "roll_modifier",
                                                      INT_MIN, INT_MAX, 2);
  const int DAMAGE = (int)lua_btech_check_integer_field(
      state, 2, "damage_modifier", INT_MIN, INT_MAX, 2);
  const bool SUCCEEDED = made_pilot_skill_roll(mech, ROLL);
  if (!SUCCEEDED)
    mech_fall(mech, DAMAGE, true);
  lua_pushboolean(state, SUCCEEDED);
  return 1;
}

static int lua_btech_unit_effective_max_speed(lua_State *state,
                                              LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package);
  const lua_Number SPEED =
      (lua_Number)mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
  lua_pushnumber(state, SPEED);
  return 1;
}

static int lua_btech_unit_section_condition(lua_State *state,
                                            LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package);
  const int SECTION = lua_btech_optional_section(state, mech, 2);
  if (SECTION < 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "section is required");
  const char *condition = "operational";
  if (mech_section_is_destroyed(mech, SECTION))
    condition = "destroyed";
  else if (mech_section_is_flooded(mech, SECTION))
    condition = "flooded";
  lua_pushstring(state, condition);
  return 1;
}

static bool optional_patch_integer(lua_State *state, int table,
                                   const char *field, int *value) {
  lua_getfield(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  if (lua_type(state, -1) != LUA_TNUMBER) {
    lua_pop(state, 1);
    (void)lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be an integer", field);
  }
  const lua_Number NUMBER = lua_tonumber(state, -1);
  lua_pop(state, 1);
  if (!isfinite(NUMBER) || floor(NUMBER) != NUMBER || NUMBER < 0 ||
      NUMBER > 255)
    (void)lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be an integer from 0 through 255", field);
  *value = (int)NUMBER;
  return true;
}

static int lua_btech_unit_set_armor(lua_State *state,
                                    LuaBtechPackage *package) {
  static const char *const FIELDS[] = {"armor", "internal", "rear_armor"};
  Mech *mech = require_mech(state, package);
  const int SECTION = lua_btech_optional_section(state, mech, 2);
  if (SECTION < 0 || mech_section_original_internal(mech, SECTION) == 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "section is required and must exist on the unit");
  lua_btech_check_options(state, 3, FIELDS, sizeof(FIELDS) / sizeof(FIELDS[0]),
                          3);
  int armor;
  int internal;
  int rear;
  const bool HAS_ARMOR = optional_patch_integer(state, 3, "armor", &armor);
  const bool HAS_INTERNAL =
      optional_patch_integer(state, 3, "internal", &internal);
  const bool HAS_REAR = optional_patch_integer(state, 3, "rear_armor", &rear);
  if (!HAS_ARMOR && !HAS_INTERNAL && !HAS_REAR)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "patch must contain at least one field");
  if (HAS_ARMOR)
    mech_section_armor_set(mech, SECTION, armor);
  if (HAS_INTERNAL)
    mech_section_internal_set(mech, SECTION, internal);
  if (HAS_REAR)
    mech_section_rear_armor_set(mech, SECTION, rear);
  return 0;
}

static int lua_btech_unit_set_max_speed(lua_State *state,
                                        LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package);
  const lua_Number SPEED = require_number(state, 2, 0, 100000, "speed");
  mech_maximum_speed_set(mech, (float)SPEED);
  mech_speed_correct(mech);
  return 0;
}

static int lua_btech_unit_set_tonnage(lua_State *state,
                                      LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package);
  const lua_Number TONS = require_number(state, 2, 1, INT_MAX / 1024, "tons");
  if (floor(TONS) != TONS)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "tons must be an integer");
  mech_tonnage_set(mech, (int)TONS);
  (void)update_oweight(mech, (int)TONS * 1024);
  return 0;
}

static void push_tic_weapon(lua_State *state, LuaBtechPackage *package,
                            Mech *mech, int number) {
  const WeaponNumberLookupResult LOOKUP = weapon_number_find(
      &(WeaponNumberLookupRequest){.mech = mech, .number = number});
  if (!LOOKUP.found)
    return;
  const int SECTION = LOOKUP.slot.section;
  const int SLOT = LOOKUP.slot.critical;
  const int PART = mech_critical_part_type(mech, SECTION, SLOT);
  const int WEAPON = weapon_from_equipment_index(PART);
  lua_newtable(state);
  lua_pushinteger(state, number);
  lua_setfield(state, -2, "number");
  const UnitSectionCatalog CATALOG = {
      .unit_type = mech_class(mech),
      .movement_type = mech_movement_type(mech),
  };
  const char *section = unit_section_name(&CATALOG, (size_t)SECTION);
  if (section == nullptr)
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "mounted weapon contains an invalid section");
  lua_pushstring(state, section);
  lua_setfield(state, -2, "section");
  lua_pushinteger(state, SLOT + 1);
  lua_setfield(state, -2, "first_slot");
  lua_btech_push_part(
      state, lua_btech_context(package),
      (PartReference){.id = PART,
                      .brand = mech_critical_brand(mech, SECTION, SLOT)});
  lua_setfield(state, -2, "part");
  lua_pushinteger(state, get_weapon_crits(mech, WEAPON));
  lua_setfield(state, -2, "slot_count");
  unsigned char weapons[MAX_WEAPS_SECTION];
  unsigned char recycle[MAX_WEAPS_SECTION];
  int criticals[MAX_WEAPS_SECTION];
  int current_recycle = 0;
  const int COUNT =
      find_weapons_advanced(mech, SECTION, weapons, recycle, criticals, 1);
  for (int index = 0; index < COUNT; index++) {
    const int CRITICAL = *(const int *)checked_storage_at_const(
        criticals, MAX_WEAPS_SECTION, sizeof(criticals[0]), (size_t)index);
    if (CRITICAL == SLOT) {
      current_recycle = *(const unsigned char *)checked_storage_at_const(
          recycle, MAX_WEAPS_SECTION, sizeof(recycle[0]), (size_t)index);
      break;
    }
  }
  lua_pushinteger(state, current_recycle);
  lua_setfield(state, -2, "recycle");
  lua_pushinteger(state, weapon_catalogue_recycle_time(WEAPON));
  lua_setfield(state, -2, "recycle_time");
  lua_pushboolean(state,
                  !weapon_is_nonfunctional(mech, SECTION, SLOT,
                                           get_weapon_crits(mech, WEAPON)));
  lua_setfield(state, -2, "operational");
}

static int lua_btech_unit_tic_weapons(lua_State *state,
                                      LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package);
  const lua_Number TIC = require_number(state, 2, 0, NUM_TICS - 1, "tic");
  if (floor(TIC) != TIC)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "tic must be an integer");
  int output = 1;
  lua_newtable(state);
  for (int number = 0; number < MAX_WEAPONS_PER_MECH; number++) {
    if (!mech_tic_contains_weapon(
            mech, (TicWeaponReference){.tic = (int)TIC, .weapon = number}))
      continue;
    const int BEFORE = lua_gettop(state);
    push_tic_weapon(state, package, mech, number);
    if (lua_gettop(state) != BEFORE)
      lua_rawseti(state, -2, output++);
  }
  return 1;
}

static const BtechLuaNativeEntry BTECH_UNIT_OPERATION_ENTRIES[] = {
    {"load_template", "unit.load_template", lua_btech_unit_load_template},
    {"piloting_check", "unit.piloting_check", lua_btech_unit_piloting_check},
    {"effective_max_speed", "unit.effective_max_speed",
     lua_btech_unit_effective_max_speed},
    {"section_condition", "unit.section_condition",
     lua_btech_unit_section_condition},
    {"set_armor", "unit.set_armor", lua_btech_unit_set_armor},
    {"set_max_speed", "unit.set_max_speed", lua_btech_unit_set_max_speed},
    {"set_tonnage", "unit.set_tonnage", lua_btech_unit_set_tonnage},
    {"tic_weapons", "unit.tic_weapons", lua_btech_unit_tic_weapons},
};

void lua_btech_install_unit_operation_bindings(lua_State *state,
                                               LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "unit", BTECH_UNIT_OPERATION_ENTRIES,
      sizeof(BTECH_UNIT_OPERATION_ENTRIES) /
          sizeof(BTECH_UNIT_OPERATION_ENTRIES[0]));
}
