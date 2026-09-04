/* btech_player_bindings.c - Lua bindings for btech.player. */

#include <lua.h>
#include <math.h>
#include <string.h>

#include "btech/configuration.h"
#include "btech/context.h"
#include "btech/repair/mechrep_api.h"
#include "btech/unit/mech_partnames_api.h"
#include "btech/unit/mech_template_api.h"
#include "btech/unit/weapon_catalogue_api.h"
#include "equipment_types.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

/**
 * @par LuaLS definition btech alias btech.player.building-contact-mode
 * @code{.lua}
 * ---@alias BtechBuildingContactMode "follow_brief"|"include"|"exclude"
 * @endcode
 *
 * @par LuaLS definition btech type btech.player.ui-preferences
 * @code{.lua}
 * ---@class BtechUiPreferencesState
 * ---@field tactical_height integer
 * ---@field tactical_width integer
 * ---@field lrs_height integer
 * ---@field include_dead boolean
 * ---@field include_shutdown boolean
 * ---@field include_enemies boolean
 * ---@field include_allies boolean
 * ---@field include_target boolean
 * ---@field buildings BtechBuildingContactMode
 * ---@field configured boolean
 * @endcode
 *
 * @par LuaLS definition btech type btech.player.personal-combat-armor
 * @code{.lua}
 * ---@class BtechPersonalCombatArmor
 * ---@field head integer
 * ---@field torso integer
 * ---@field hands integer
 * ---@field feet integer
 * @endcode
 *
 * @par LuaLS definition btech type btech.player.personal-combat-equipment
 * @code{.lua}
 * ---@class BtechPersonalCombatEquipment
 * ---@field weapon BtechPart
 * ---@field ammunition? integer
 * @endcode
 *
 * @par LuaLS definition btech type btech.player.personal-combat-loadout
 * @code{.lua}
 * ---@class BtechPersonalCombatLoadout
 * ---@field armor BtechPersonalCombatArmor
 * ---@field right? BtechPersonalCombatEquipment
 * ---@field left? BtechPersonalCombatEquipment
 * @endcode
 *
 * @par Lua API definition btech namespace btech.player
 * @code{.lua}
 * ---Player-owned BattleTech configuration.
 * ---@class BtechPlayerPackage
 * local btech_player = {}
 * @endcode
 */

static DbRef require_player(lua_State *state, LuaBtechPackage *package,
                            int argument) {
  const DbRef PLAYER = lua_btech_require_object(package, state, argument);
  if (!is_player(package->services->database, PLAYER) ||
      is_going(package->services->database, PLAYER))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                        "object is not a live player");
  return PLAYER;
}

static void set_integer_field(lua_State *state, const char *field, int value) {
  lua_pushinteger(state, value);
  lua_setfield(state, -2, field);
}

static void set_boolean_field(lua_State *state, const char *field, bool value) {
  lua_pushboolean(state, (int)value);
  lua_setfield(state, -2, field);
}

static const char *building_mode_name(BtechBuildingContactMode mode) {
  switch (mode) {
  case BTECH_BUILDING_CONTACTS_FOLLOW_BRIEF:
    return "follow_brief";
  case BTECH_BUILDING_CONTACTS_INCLUDE:
    return "include";
  case BTECH_BUILDING_CONTACTS_EXCLUDE:
    return "exclude";
  }
  return "exclude";
}

static BtechBuildingContactMode check_building_mode(lua_State *state,
                                                    int table) {
  const char *value = lua_btech_check_string_field(
      state, table, "buildings", sizeof("follow_brief") - 1, 2);
  if (strcmp(value, "follow_brief") == 0)
    return BTECH_BUILDING_CONTACTS_FOLLOW_BRIEF;
  if (strcmp(value, "include") == 0)
    return BTECH_BUILDING_CONTACTS_INCLUDE;
  if (strcmp(value, "exclude") == 0)
    return BTECH_BUILDING_CONTACTS_EXCLUDE;
  (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                      "buildings has an invalid mode");
  return BTECH_BUILDING_CONTACTS_EXCLUDE;
}

static void push_ui_preferences(lua_State *state,
                                BtechPlayerUiPreferences preferences) {
  lua_newtable(state);
  set_integer_field(state, "tactical_height", preferences.tactical_height);
  set_integer_field(state, "tactical_width", preferences.tactical_width);
  set_integer_field(state, "lrs_height", preferences.lrs_height);
  set_boolean_field(state, "include_dead", preferences.include_dead);
  set_boolean_field(state, "include_shutdown", preferences.include_shutdown);
  set_boolean_field(state, "include_enemies", preferences.include_enemies);
  set_boolean_field(state, "include_allies", preferences.include_allies);
  set_boolean_field(state, "include_target", preferences.include_target);
  lua_pushstring(state, building_mode_name(preferences.buildings));
  lua_setfield(state, -2, "buildings");
}

static BtechPlayerUiPreferences check_ui_preferences(lua_State *state,
                                                     int table) {
  static const char *const FIELDS[] = {
      "tactical_height", "tactical_width",   "lrs_height",
      "include_dead",    "include_shutdown", "include_enemies",
      "include_allies",  "include_target",   "buildings",
  };
  lua_btech_check_options(state, table, FIELDS,
                          sizeof(FIELDS) / sizeof(FIELDS[0]), 2);
  return (BtechPlayerUiPreferences){
      .tactical_height = (int)lua_btech_check_integer_field(
          state, table, "tactical_height", 5, 24, 2),
      .tactical_width = (int)lua_btech_check_integer_field(
          state, table, "tactical_width", 5, 40, 2),
      .lrs_height = (int)lua_btech_check_integer_field(state, table,
                                                       "lrs_height", 10, 40, 2),
      .include_dead =
          lua_btech_check_boolean_field(state, table, "include_dead", 2),
      .include_shutdown =
          lua_btech_check_boolean_field(state, table, "include_shutdown", 2),
      .include_enemies =
          lua_btech_check_boolean_field(state, table, "include_enemies", 2),
      .include_allies =
          lua_btech_check_boolean_field(state, table, "include_allies", 2),
      .include_target =
          lua_btech_check_boolean_field(state, table, "include_target", 2),
      .buildings = check_building_mode(state, table),
  };
}

/**
 * @par Lua API definition btech callable btech.player.ui_preferences
 * @code{.lua}
 * ---Returns effective UI preferences and whether they are explicitly configured.
 * ---@param player DbRef|Object
 * ---@return BtechUiPreferencesState preferences
 * function btech_player.ui_preferences(player) end
 * @endcode
 */
static int lua_btech_player_ui_preferences(lua_State *state,
                                           LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef PLAYER = require_player(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  push_ui_preferences(state, btech_player_ui_preferences(context, PLAYER));
  const bool CONFIGURED =
      btech_player_ui_preferences_configured(context, PLAYER);
  lua_pushboolean(state, (int)CONFIGURED);
  lua_setfield(state, -2, "configured");
  return 1;
}

/**
 * @par Lua API definition btech callable btech.player.set_ui_preferences
 * @code{.lua}
 * ---Atomically sets UI preferences, or restores defaults with nil.
 * ---@param player DbRef|Object
 * ---@param preferences BtechUiPreferencesState|nil
 * function btech_player.set_ui_preferences(player, preferences) end
 * @endcode
 */
static int lua_btech_player_set_ui_preferences(lua_State *state,
                                               LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef PLAYER = require_player(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  if (lua_isnil(state, 2)) {
    btech_player_ui_preferences_clear(context, PLAYER);
  } else {
    BtechPlayerUiPreferences preferences = check_ui_preferences(state, 2);
    if (!btech_player_ui_preferences_set(context, PLAYER, preferences))
      return lua_btech_operation_error(state, "ui_preferences_store_failed",
                                       "unable to store UI preferences");
  }
  return 0;
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
 * @par Lua API definition btech callable btech.player.mechwarrior_template
 * @code{.lua}
 * ---Returns the configured MechWarrior template override, or nil.
 * ---@param player DbRef|Object
 * ---@return string|nil reference
 * function btech_player.mechwarrior_template(player) end
 * @endcode
 */
static int lua_btech_player_mechwarrior_template(lua_State *state,
                                                 LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef PLAYER = require_player(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  if (!btech_player_mechwarrior_template_configured(context, PLAYER))
    lua_pushnil(state);
  else
    lua_pushstring(state, btech_player_mechwarrior_template(context, PLAYER));
  return 1;
}

/**
 * @par Lua API definition btech callable btech.player.set_mechwarrior_template
 * @code{.lua}
 * ---Sets the MechWarrior template override, or clears it with nil.
 * ---@param player DbRef|Object
 * ---@param reference string|nil
 * function btech_player.set_mechwarrior_template(player, reference) end
 * @endcode
 */
static int lua_btech_player_set_mechwarrior_template(lua_State *state,
                                                     LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef PLAYER = require_player(state, package, 1);
  const char *reference = optional_string(state, 2, 24, "reference");
  BtechContext *context = lua_btech_context(package);
  if (reference != nullptr) {
    lua_btech_validate_resource_name(state, 2, reference, "reference");
    if (mech_template_resolve_path(context,
                                   btech_context_mech_template_path(context),
                                   reference) == nullptr)
      return lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_TEMPLATE_NOT_FOUND,
                           "template was not found");
    if (load_refmech(context, reference) == nullptr)
      return lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_TEMPLATE_INVALID,
                           "template is malformed");
  }
  if (!btech_player_mechwarrior_template_set(context, PLAYER, reference))
    return lua_btech_operation_error(state, "template_preference_store_failed",
                                     "unable to store MechWarrior template");
  return 0;
}

static bool check_optional_ammunition(lua_State *state, int table,
                                      int *ammunition) {
  lua_btech_get_field(state, table, "ammunition");
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    *ammunition = 0;
    return false;
  }
  if (lua_type(state, -1) != LUA_TNUMBER)
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "ammunition must be an integer from 0 to 255");
  const lua_Number NUMBER = lua_tonumber(state, -1);
  if (!isfinite(NUMBER) || floor(NUMBER) != NUMBER || NUMBER < 0 ||
      NUMBER > 255)
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "ammunition must be an integer from 0 to 255");
  lua_pop(state, 1);
  *ammunition = (int)NUMBER;
  return true;
}

static void check_equipment(lua_State *state, LuaBtechPackage *package,
                            int loadout, const char *field, char *weapon,
                            size_t weapon_size, bool *has_ammunition,
                            int *ammunition) {
  static const char *const FIELDS[] = {"weapon", "ammunition"};
  lua_btech_get_field(state, loadout, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    *weapon = '\0';
    *has_ammunition = false;
    *ammunition = 0;
    return;
  }
  if (!lua_istable(state, -1))
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "%s must be a table or nil", field);
  const int TABLE = lua_gettop(state);
  lua_btech_check_options(state, TABLE, FIELDS,
                          sizeof(FIELDS) / sizeof(FIELDS[0]), 2);
  lua_btech_get_field(state, TABLE, "weapon");
  PartReference part;
  if (!lua_btech_check_part(state, lua_btech_context(package), -1, 2, &part))
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_PART_NOT_FOUND,
                        "%s.weapon was not found", field);
  lua_pop(state, 1);
  if (!equipment_is_weapon(part.id))
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_PART_WRONG_KIND,
                        "%s.weapon is not a weapon", field);
  const int WEAPON = weapon_from_equipment_index(part.id);
  if (!weapon_catalogue_is_personal_combat(WEAPON))
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_BTECH_PART_WRONG_KIND,
                        "%s.weapon is not a personal-combat weapon", field);
  (void)string_copy_bounded(
      weapon, weapon_size,
      get_parts_vlong_name(lua_btech_context(package), part.id, 0));
  *has_ammunition = check_optional_ammunition(state, TABLE, ammunition);
  if (*has_ammunition && weapon_catalogue_ammunition_per_ton(WEAPON) <= 0)
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "%s.ammunition is invalid for this weapon", field);
  lua_pop(state, 1);
}

static BtechPersonalCombatLoadout
check_loadout(lua_State *state, LuaBtechPackage *package, int table) {
  static const char *const FIELDS[] = {"armor", "right", "left"};
  static const char *const ARMOR_FIELDS[] = {"head", "torso", "hands", "feet"};
  BtechPersonalCombatLoadout loadout = {0};

  lua_btech_check_options(state, table, FIELDS,
                          sizeof(FIELDS) / sizeof(FIELDS[0]), 2);
  lua_btech_get_field(state, table, "armor");
  if (!lua_istable(state, -1))
    (void)lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                        "armor must be a table");
  const int ARMOR_TABLE = lua_gettop(state);
  lua_btech_check_options(state, ARMOR_TABLE, ARMOR_FIELDS,
                          sizeof(ARMOR_FIELDS) / sizeof(ARMOR_FIELDS[0]), 2);
  loadout.armor_head =
      (int)lua_btech_check_integer_field(state, ARMOR_TABLE, "head", 0, 2, 2);
  loadout.armor_torso =
      (int)lua_btech_check_integer_field(state, ARMOR_TABLE, "torso", 0, 8, 2);
  loadout.armor_hands =
      (int)lua_btech_check_integer_field(state, ARMOR_TABLE, "hands", 0, 2, 2);
  loadout.armor_feet =
      (int)lua_btech_check_integer_field(state, ARMOR_TABLE, "feet", 0, 2, 2);
  lua_pop(state, 1);

  check_equipment(state, package, table, "right", loadout.right_weapon,
                  sizeof(loadout.right_weapon), &loadout.has_right_ammunition,
                  &loadout.right_ammunition);
  check_equipment(state, package, table, "left", loadout.left_weapon,
                  sizeof(loadout.left_weapon), &loadout.has_left_ammunition,
                  &loadout.left_ammunition);
  return loadout;
}

static void push_equipment(lua_State *state, BtechContext *context,
                           const char *field, bool has_ammunition,
                           const char *weapon, int ammunition) {
  if (*weapon == '\0')
    return;
  lua_newtable(state);
  lua_pushstring(state, weapon);
  PartReference part;
  if (!lua_btech_check_part(state, context, -1, 1, &part))
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "configured loadout contains an unknown weapon");
  lua_pop(state, 1);
  lua_btech_push_part(state, context,
                      (PartReference){.id = part.id, .brand = 0});
  lua_setfield(state, -2, "weapon");
  if (has_ammunition) {
    lua_pushinteger(state, ammunition);
    lua_setfield(state, -2, "ammunition");
  }
  lua_setfield(state, -2, field);
}

static void push_loadout(lua_State *state, BtechContext *context,
                         const BtechPersonalCombatLoadout *loadout) {
  lua_newtable(state);
  lua_newtable(state);
  set_integer_field(state, "head", loadout->armor_head);
  set_integer_field(state, "torso", loadout->armor_torso);
  set_integer_field(state, "hands", loadout->armor_hands);
  set_integer_field(state, "feet", loadout->armor_feet);
  lua_setfield(state, -2, "armor");
  push_equipment(state, context, "right", loadout->has_right_ammunition,
                 loadout->right_weapon, loadout->right_ammunition);
  push_equipment(state, context, "left", loadout->has_left_ammunition,
                 loadout->left_weapon, loadout->left_ammunition);
}

/**
 * @par Lua API definition btech callable btech.player.loadout
 * @code{.lua}
 * ---Returns the configured personal-combat loadout, or nil.
 * ---@param player DbRef|Object
 * ---@return BtechPersonalCombatLoadout|nil loadout
 * function btech_player.loadout(player) end
 * @endcode
 */
static int lua_btech_player_loadout(lua_State *state,
                                    LuaBtechPackage *package) {
  lua_btech_check_arity(state, 1);
  const DbRef PLAYER = require_player(state, package, 1);
  BtechPersonalCombatLoadout loadout;
  if (!btech_player_loadout(lua_btech_context(package), PLAYER, &loadout))
    lua_pushnil(state);
  else
    push_loadout(state, lua_btech_context(package), &loadout);
  return 1;
}

/**
 * @par Lua API definition btech callable btech.player.set_loadout
 * @code{.lua}
 * ---Atomically sets a personal-combat loadout, or clears it with nil.
 * ---@param player DbRef|Object
 * ---@param loadout BtechPersonalCombatLoadout|nil
 * function btech_player.set_loadout(player, loadout) end
 * @endcode
 */
static int lua_btech_player_set_loadout(lua_State *state,
                                        LuaBtechPackage *package) {
  lua_btech_check_arity(state, 2);
  const DbRef PLAYER = require_player(state, package, 1);
  BtechContext *context = lua_btech_context(package);
  if (lua_isnil(state, 2)) {
    btech_player_loadout_clear(context, PLAYER);
  } else {
    BtechPersonalCombatLoadout loadout = check_loadout(state, package, 2);
    if (!btech_player_loadout_set(context, PLAYER, &loadout))
      return lua_btech_operation_error(
          state, "loadout_store_failed",
          "unable to store personal-combat loadout");
  }
  return 0;
}

static const BtechLuaNativeEntry BTECH_PLAYER_NATIVE_ENTRIES[] = {
    {"ui_preferences", "player.ui_preferences",
     lua_btech_player_ui_preferences},
    {"set_ui_preferences", "player.set_ui_preferences",
     lua_btech_player_set_ui_preferences},
    {"mechwarrior_template", "player.mechwarrior_template",
     lua_btech_player_mechwarrior_template},
    {"set_mechwarrior_template", "player.set_mechwarrior_template",
     lua_btech_player_set_mechwarrior_template},
    {"loadout", "player.loadout", lua_btech_player_loadout},
    {"set_loadout", "player.set_loadout", lua_btech_player_set_loadout},
};

void lua_btech_install_player_bindings(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_install_native_bindings(state, package, "player",
                                    BTECH_PLAYER_NATIVE_ENTRIES,
                                    sizeof(BTECH_PLAYER_NATIVE_ENTRIES) /
                                        sizeof(BTECH_PLAYER_NATIVE_ENTRIES[0]));
}
