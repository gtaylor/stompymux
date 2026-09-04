/* btech_parts_bindings.c - Native typed Lua bindings for btech.parts. */

#include <limits.h>
#include <lua.h>
#include <math.h>
#include <stddef.h>
#include <string.h>
#include <strings.h>

#include "btech/economy/part_cost_api.h"
#include "btech/economy/unit_cost_api.h"
#include "btech/unit/equipment_types.h"
#include "btech/unit/mech_partnames_api.h"
#include "btech/unit/weapon_catalogue_api.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/economy_parts.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/wild.h"

static constexpr unsigned long long LUA_SAFE_INTEGER_MAX = 9007199254740991ULL;

static const char *part_category(int id) {
  if (equipment_is_weapon(id))
    return "weapon";
  if (equipment_is_ammunition(id))
    return "ammunition";
  if (equipment_is_bomb(id))
    return "bomb";
  if (equipment_is_special(id))
    return "special";
  if (equipment_is_cargo(id))
    return "cargo";
  return "other";
}

static bool category_matches(const char *category, int id) {
  return (bool)(category == nullptr ||
                strcmp(category, part_category(id)) == 0);
}

static void set_integer(lua_State *state, const char *field,
                        lua_Integer value) {
  lua_pushinteger(state, value);
  lua_setfield(state, -2, field);
}

static void set_number(lua_State *state, const char *field, lua_Number value) {
  lua_pushnumber(state, value);
  lua_setfield(state, -2, field);
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters): Field/value pairs are the
// idiomatic interface for these small Lua record builders.
static void set_string(lua_State *state, const char *field, const char *value) {
  lua_pushstring(state, value);
  lua_setfield(state, -2, field);
}
// NOLINTEND(bugprone-easily-swappable-parameters)

static const char *weapon_kind(int weapon) {
  if (weapon_catalogue_is_artillery(weapon))
    return "artillery";
  if (weapon_catalogue_is_missile(weapon))
    return "missile";
  if (weapon_catalogue_is_ballistic(weapon))
    return "ballistic";
  if (weapon_catalogue_is_energy(weapon))
    return "energy";
  if (weapon_catalogue_is_hand_to_hand(weapon))
    return "melee";
  return "other";
}

static void push_weapon(lua_State *state, int id) {
  const int WEAPON = id - WEAPON_BASE_INDEX;
  const WeaponRangeProfile RANGES = weapon_catalogue_ranges(WEAPON);
  lua_newtable(state);
  set_string(state, "kind", weapon_kind(WEAPON));
  set_integer(state, "heat", weapon_catalogue_heat(WEAPON));
  set_integer(state, "damage", weapon_catalogue_damage(WEAPON));
  set_integer(state, "minimum_range", RANGES.minimum);
  set_integer(state, "short_range", RANGES.short_range);
  set_integer(state, "medium_range", RANGES.medium_range);
  set_integer(state, "long_range", RANGES.long_range);
  set_integer(state, "critical_slots", weapon_catalogue_critical_slots(WEAPON));
  set_integer(state, "ammunition_per_ton",
              weapon_catalogue_ammunition_per_ton(WEAPON));
  set_integer(state, "recycle_time", weapon_catalogue_recycle_time(WEAPON));
  set_integer(state, "battle_value", weapon_catalogue_battle_value(WEAPON));
}

static bool part_is_registered(BtechContext *context, PartReference part) {
  return (bool)(part.id >= 0 && part.id < NUM_ITEMS && part.brand >= 0 &&
                part.brand <= BRANDCOUNT &&
                get_parts_short_name(context, part.id, part.brand) != nullptr);
}

void lua_btech_push_part(lua_State *state, BtechContext *context,
                         PartReference part) {
  const unsigned long long COST = btech_part_cost_get(context, part.id);
  if (COST > LUA_SAFE_INTEGER_MAX)
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "part cost is not representable in Lua");
  lua_newtable(state);
  set_integer(state, "id", part.id);
  set_integer(state, "brand", part.brand);
  set_integer(state, "packed_id", (part.brand * NUM_ITEMS) + part.id);
  set_string(state, "short_name",
             get_parts_short_name(context, part.id, part.brand));
  set_string(state, "long_name",
             get_parts_long_name(context, part.id, part.brand));
  set_string(state, "very_long_name",
             get_parts_vlong_name(context, part.id, part.brand));
  set_string(state, "category", part_category(part.id));
  const int WEIGHT = btech_part_weight(part.id);
  set_number(state, "weight_tons", (lua_Number)WEIGHT / 1024.0);
  set_integer(state, "cost", (lua_Integer)COST);
  if (equipment_is_weapon(part.id)) {
    push_weapon(state, part.id);
    lua_setfield(state, -2, "weapon");
  }
}

static bool exact_name_matches(BtechContext *context, PartReference part,
                               const char *name) {
  return (bool)(strcasecmp(name, get_parts_short_name(context, part.id,
                                                      part.brand)) == 0 ||
                strcasecmp(name, get_parts_long_name(context, part.id,
                                                     part.brand)) == 0 ||
                strcasecmp(name, get_parts_vlong_name(context, part.id,
                                                      part.brand)) == 0);
}

static bool find_named_part(lua_State *state, BtechContext *context,
                            const char *name, int argument,
                            PartReference *part) {
  bool found = false;
  for (size_t index = 0; index < part_name_count(context); index++) {
    const PartReference CANDIDATE =
        part_name_reference(part_name_at(context, index));
    if (!exact_name_matches(context, CANDIDATE, name))
      continue;
    if (found && (part->id != CANDIDATE.id || part->brand != CANDIDATE.brand))
      (void)lua_error_arg(state, argument, LUA_ERROR_CODE_BTECH_PART_AMBIGUOUS,
                          "part name is ambiguous");
    *part = CANDIDATE;
    found = true;
  }
  return found;
}

bool lua_btech_check_part(lua_State *state, BtechContext *context, int index,
                          int argument, PartReference *part) {
  if (lua_type(state, index) == LUA_TNUMBER) {
    const lua_Number NUMBER = lua_tonumber(state, index);
    if (!isfinite(NUMBER) || floor(NUMBER) != NUMBER || NUMBER < 0 ||
        NUMBER > INT_MAX)
      (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                          "packed part ID must be a nonnegative integer");
    const int PACKED = (int)NUMBER;
    *part =
        (PartReference){.id = PACKED % NUM_ITEMS, .brand = PACKED / NUM_ITEMS};
  } else if (lua_type(state, index) == LUA_TSTRING) {
    return find_named_part(state, context, lua_tostring(state, index), argument,
                           part);
  } else if (lua_istable(state, index)) {
    part->id = (int)lua_btech_check_integer_field(state, index, "id", 0,
                                                  NUM_ITEMS - 1, argument);
    part->brand = (int)lua_btech_check_integer_field(state, index, "brand", 0,
                                                     BRANDCOUNT, argument);
  } else {
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "part must be a packed ID, name, or part record");
  }
  return part_is_registered(context, *part);
}

static const char *optional_category(lua_State *state, int argument) {
  if (lua_isnoneornil(state, argument))
    return nullptr;
  if (lua_type(state, argument) != LUA_TSTRING)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "category must be a string");
  const char *category = lua_tostring(state, argument);
  static const char *const CATEGORIES[] = {"weapon",  "ammunition", "bomb",
                                           "special", "cargo",      "other"};
  for (size_t index = 0; index < sizeof(CATEGORIES) / sizeof(CATEGORIES[0]);
       index++) {
    const char *candidate = *(const char *const *)checked_storage_at_const(
        (const void *)CATEGORIES, sizeof(CATEGORIES) / sizeof(CATEGORIES[0]),
        sizeof(CATEGORIES[0]), index);
    if (strcasecmp(category, candidate) == 0)
      return candidate;
  }
  (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                      "unknown part category");
  return nullptr;
}

static int lua_btech_parts_categories(lua_State *state, LuaBtechPackage *package
                                      [[maybe_unused]]) {
  static const char *const CATEGORIES[][2] = {
      {"weapon", "Weapons"}, {"ammunition", "Ammunition"},
      {"bomb", "Bombs"},     {"special", "Special Equipment"},
      {"cargo", "Cargo"},    {"other", "Other"},
  };
  lua_newtable(state);
  for (size_t index = 0; index < sizeof(CATEGORIES) / sizeof(CATEGORIES[0]);
       index++) {
    const char *const *category = (const char *const *)checked_storage_at_const(
        (const void *)CATEGORIES, sizeof(CATEGORIES) / sizeof(CATEGORIES[0]),
        sizeof(CATEGORIES[0]), index);
    lua_newtable(state);
    set_string(state, "code", *category);
    set_string(state, "name",
               *(const char *const *)checked_storage_at_const(
                   (const void *)category, 2, sizeof(*category), 1));
    lua_rawseti(state, -2, (int)index + 1);
  }
  return 1;
}

static int lua_btech_parts_list(lua_State *state, LuaBtechPackage *package) {
  BtechContext *context = lua_btech_context(package);
  const char *category = optional_category(state, 1);
  int output = 1;
  lua_newtable(state);
  for (size_t index = 0; index < part_name_count(context); index++) {
    const PartReference PART =
        part_name_reference(part_name_at(context, index));
    if (!category_matches(category, PART.id))
      continue;
    lua_btech_push_part(state, context, PART);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static int lua_btech_parts_search(lua_State *state, LuaBtechPackage *package) {
  if (lua_type(state, 1) != LUA_TSTRING || *lua_tostring(state, 1) == '\0')
    return lua_error_arg(state, 1, LUA_ERROR_CODE_ARG_INVALID,
                         "query must be a non-empty string");
  BtechContext *context = lua_btech_context(package);
  const char *query = lua_tostring(state, 1);
  int output = 1;
  lua_newtable(state);
  for (size_t index = 0; index < part_name_count(context); index++) {
    const PartReference PART =
        part_name_reference(part_name_at(context, index));
    if (!quick_wild(query,
                    get_parts_short_name(context, PART.id, PART.brand)) &&
        !quick_wild(query, get_parts_long_name(context, PART.id, PART.brand)) &&
        !quick_wild(query, get_parts_vlong_name(context, PART.id, PART.brand)))
      continue;
    lua_btech_push_part(state, context, PART);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static int lua_btech_parts_resolve(lua_State *state, LuaBtechPackage *package) {
  PartReference part = {0};
  BtechContext *context = lua_btech_context(package);
  if (!lua_btech_check_part(state, context, 1, 1, &part))
    lua_pushnil(state);
  else
    lua_btech_push_part(state, context, part);
  return 1;
}

static int lua_btech_parts_stores(lua_State *state, LuaBtechPackage *package) {
  const DbRef TARGET = lua_btech_require_object(package, state, 1);
  BtechContext *context = lua_btech_context(package);
  int output = 1;
  lua_newtable(state);
  for (size_t index = 0;
       index < economy_parts_entry_count(package->services->database, TARGET);
       index++) {
    const EconomyPartsEntryResult RESULT = economy_parts_entry(
        &(EconomyPartsEntryRequest){.database = package->services->database,
                                    .object = TARGET,
                                    .index = index});
    const PartReference PART = {.id = RESULT.entry.part_id,
                                .brand = RESULT.entry.brand_id};
    if (!RESULT.found || RESULT.entry.quantity <= 0 ||
        !part_is_registered(context, PART))
      continue;
    lua_newtable(state);
    lua_btech_push_part(state, context, PART);
    lua_setfield(state, -2, "part");
    set_integer(state, "quantity", RESULT.entry.quantity);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

static int lua_btech_parts_store_quantity(lua_State *state,
                                          LuaBtechPackage *package) {
  const DbRef TARGET = lua_btech_require_object(package, state, 1);
  PartReference part = {0};
  if (!lua_btech_check_part(state, lua_btech_context(package), 2, 2, &part))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_PART_NOT_FOUND,
                         "part is not registered");
  lua_pushinteger(state, economy_parts_quantity(package->services->database,
                                                TARGET, part.id, part.brand));
  return 1;
}

static int lua_btech_parts_adjust_stores(lua_State *state,
                                         LuaBtechPackage *package) {
  const DbRef TARGET = lua_btech_require_object(package, state, 1);
  PartReference part = {0};
  if (!lua_btech_check_part(state, lua_btech_context(package), 2, 2, &part))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_PART_NOT_FOUND,
                         "part is not registered");
  if (lua_type(state, 3) != LUA_TNUMBER)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "delta must be a nonzero integer");
  const lua_Number DELTA_NUMBER = lua_tonumber(state, 3);
  if (!isfinite(DELTA_NUMBER) || floor(DELTA_NUMBER) != DELTA_NUMBER ||
      DELTA_NUMBER < INT_MIN || DELTA_NUMBER > INT_MAX ||
      (DELTA_NUMBER > -1 && DELTA_NUMBER < 1))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "delta must be a nonzero ranged integer");
  const int CURRENT = economy_parts_quantity(package->services->database,
                                             TARGET, part.id, part.brand);
  const long long UPDATED = (long long)CURRENT + (long long)DELTA_NUMBER;
  if (UPDATED < 0 || UPDATED > INT_MAX)
    return lua_btech_operation_error(state, "store_capacity_exceeded",
                                     "store adjustment would exceed capacity");
  if (!economy_parts_set_quantity(package->services->database, TARGET, part.id,
                                  part.brand, (int)UPDATED))
    return lua_btech_operation_error(state, "store_commit_failed",
                                     "store adjustment could not be committed");
  return 0;
}

static int lua_btech_parts_set_cost(lua_State *state,
                                    LuaBtechPackage *package) {
  PartReference part = {0};
  if (!lua_btech_check_part(state, lua_btech_context(package), 1, 1, &part))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_BTECH_PART_NOT_FOUND,
                         "part is not registered");
  if (lua_type(state, 2) != LUA_TNUMBER)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "cost must be a safe nonnegative integer");
  const lua_Number COST = lua_tonumber(state, 2);
  if (!isfinite(COST) || floor(COST) != COST || COST < 0 ||
      COST > (lua_Number)LUA_SAFE_INTEGER_MAX)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "cost must be an integer from 0 to 2^53-1");
  btech_part_cost_set(lua_btech_context(package), part.id,
                      (unsigned long long)COST);
  return 0;
}

static const BtechLuaNativeEntry BTECH_PARTS_ENTRIES[] = {
    {"categories", "parts.categories", lua_btech_parts_categories},
    {"list", "parts.list", lua_btech_parts_list},
    {"search", "parts.search", lua_btech_parts_search},
    {"resolve", "parts.resolve", lua_btech_parts_resolve},
    {"stores", "parts.stores", lua_btech_parts_stores},
    {"store_quantity", "parts.store_quantity", lua_btech_parts_store_quantity},
    {"adjust_stores", "parts.adjust_stores", lua_btech_parts_adjust_stores},
    {"set_cost", "parts.set_cost", lua_btech_parts_set_cost},
};

void lua_btech_install_parts_bindings(lua_State *state,
                                      LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "parts", BTECH_PARTS_ENTRIES,
      sizeof(BTECH_PARTS_ENTRIES) / sizeof(BTECH_PARTS_ENTRIES[0]));
}
