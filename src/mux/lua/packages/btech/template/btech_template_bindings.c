/* btech_template_bindings.c - Lua bindings for btech.template. */

#include <lua.h>
#include <stddef.h>

#include "btech/context.h"
#include "btech/economy/unit_cost_api.h"
#include "btech/repair/mechrep_api.h"
#include "btech/ui/mech_status_api.h"
#include "btech/unit/mech_classification_api.h"
#include "btech/unit/mech_consistency_api.h"
#include "btech/unit/mech_specification_api.h"
#include "btech/unit/mech_template_api.h"
#include "btech/unit/mech_utils_api.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"

/**
 * @par Lua API definition btech namespace btech.template
 * @code{.lua}
 * ---Immutable unit-template queries and displays.
 * ---@class BtechTemplatePackage
 * local btech_template = {}
 * @endcode
 */

static const char *require_reference(lua_State *state, int argument) {
  size_t length;
  if (lua_type(state, argument) != LUA_TSTRING)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "reference must be a string");
  const char *reference = lua_tolstring(state, argument, &length);
  if (length == 0)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "reference must not be empty");
  lua_btech_validate_resource_name(state, argument, reference, "reference");
  return reference;
}

/**
 * @par Lua API definition btech callable btech.template.exists
 * @code{.lua}
 * ---Returns false for a missing template and validates an existing template.
 * ---@param reference string
 * ---@return boolean exists
 * function btech_template.exists(reference) end
 * @endcode
 */
static int lua_btech_template_exists(lua_State *state,
                                     LuaBtechPackage *package) {
  const char *reference = require_reference(state, 1);
  BtechContext *context = lua_btech_context(package);
  const char *path = mech_template_resolve_path(
      context, btech_context_mech_template_path(context), reference);

  if (path == nullptr) {
    lua_pushboolean(state, 0);
    return 1;
  }
  if (load_refmech(context, reference) == nullptr)
    return lua_error_arg(state, 1, LUA_ERROR_CODE_BTECH_TEMPLATE_INVALID,
                         "existing template is malformed");
  lua_pushboolean(state, 1);
  return 1;
}

static Mech *require_template(lua_State *state, LuaBtechPackage *package,
                              int argument) {
  const char *reference = require_reference(state, argument);
  BtechContext *context = lua_btech_context(package);
  if (mech_template_resolve_path(context,
                                 btech_context_mech_template_path(context),
                                 reference) == nullptr)
    (void)lua_error_arg(state, argument,
                        LUA_ERROR_CODE_BTECH_TEMPLATE_NOT_FOUND,
                        "template was not found");
  Mech *mech = load_refmech(context, reference);
  if (mech == nullptr)
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_BTECH_TEMPLATE_INVALID,
                        "template is malformed");
  return mech;
}

static int lua_btech_template_engine(lua_State *state,
                                     LuaBtechPackage *package) {
  Mech *mech = require_template(state, package, 1);
  lua_newtable(state);
  lua_pushinteger(state, mech_engine_rating(mech));
  lua_setfield(state, -2, "rating");
  lua_pushinteger(state, susp_factor(mech));
  lua_setfield(state, -2, "suspension_factor");
  return 1;
}

static int lua_btech_template_battle_value(lua_State *state,
                                           LuaBtechPackage *package) {
  if (lua_gettop(state) != 1)
    return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                           "expected exactly 1 argument");
  Mech *mech = require_template(state, package, 1);
  lua_btech_push_battle_value(state, mech);
  return 1;
}

static int lua_btech_template_base_cost(lua_State *state,
                                        LuaBtechPackage *package) {
  constexpr unsigned long long MAXIMUM = 9007199254740991ULL;
  const unsigned long long COST =
      mech_fasa_cost(require_template(state, package, 1));
  if (COST > MAXIMUM)
    return lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                           "template base cost is not representable in Lua");
  lua_pushinteger(state, (lua_Integer)COST);
  return 1;
}

static int lua_btech_template_payload(lua_State *state,
                                      LuaBtechPackage *package) {
  lua_btech_push_payload(state, lua_btech_context(package),
                         require_template(state, package, 1));
  return 1;
}

static int lua_btech_template_armor(lua_State *state,
                                    LuaBtechPackage *package) {
  Mech *mech = require_template(state, package, 1);
  lua_btech_push_armor(state, mech, lua_btech_optional_section(state, mech, 2));
  return 1;
}

static int lua_btech_template_installed_parts(lua_State *state,
                                              LuaBtechPackage *package) {
  lua_btech_push_installed_parts(state, lua_btech_context(package),
                                 require_template(state, package, 1));
  return 1;
}

static int lua_btech_template_critical_slots(lua_State *state,
                                             LuaBtechPackage *package) {
  Mech *mech = require_template(state, package, 1);
  const int SECTION = lua_btech_optional_section(state, mech, 2);
  if (SECTION < 0)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "section is required");
  lua_btech_push_critical_slots(state, lua_btech_context(package), mech,
                                SECTION);
  return 1;
}

static int lua_btech_template_weapons(lua_State *state,
                                      LuaBtechPackage *package) {
  Mech *mech = require_template(state, package, 1);
  lua_btech_push_weapons(state, lua_btech_context(package), mech,
                         lua_btech_optional_section(state, mech, 2));
  return 1;
}

static int lua_btech_template_technologies(lua_State *state,
                                           LuaBtechPackage *package) {
  lua_btech_push_technologies(state, require_template(state, package, 1));
  return 1;
}

static DbRef require_player(lua_State *state, LuaBtechPackage *package,
                            int argument) {
  const DbRef PLAYER = lua_btech_require_object(package, state, argument);
  if (!is_player(package->services->database, PLAYER))
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                        "display recipient must be a player");
  return PLAYER;
}

static int lua_btech_template_show_status(lua_State *state,
                                          LuaBtechPackage *package) {
  Mech *mech = require_template(state, package, 1);
  mech_status(require_player(state, package, 2), mech, "R");
  return 0;
}

static int lua_btech_template_show_weapon_specs(lua_State *state,
                                                LuaBtechPackage *package) {
  Mech *mech = require_template(state, package, 1);
  mech_weaponspecs(require_player(state, package, 2), mech, "");
  return 0;
}

static int lua_btech_template_show_critical_status(lua_State *state,
                                                   LuaBtechPackage *package) {
  Mech *mech = require_template(state, package, 1);
  const DbRef PLAYER = require_player(state, package, 2);
  const int SECTION = lua_btech_optional_section(state, mech, 3);
  if (SECTION < 0)
    return lua_error_arg(state, 3, LUA_ERROR_CODE_ARG_INVALID,
                         "section is required");
  const UnitSectionCatalog CATALOG = {
      .unit_type = mech_class(mech),
      .movement_type = mech_movement_type(mech),
  };
  char section[UNIT_SECTION_NAME_CAPACITY];
  (void)string_copy_bounded(section, sizeof(section),
                            unit_section_name(&CATALOG, (size_t)SECTION));
  mech_critstatus(PLAYER, mech, section);
  return 0;
}

static const BtechLuaNativeEntry BTECH_TEMPLATE_ENTRIES[] = {
    {"exists", "template.exists", lua_btech_template_exists},
    {"engine", "template.engine", lua_btech_template_engine},
    {"battle_value", "template.battle_value", lua_btech_template_battle_value},
    {"base_cost", "template.base_cost", lua_btech_template_base_cost},
    {"payload", "template.payload", lua_btech_template_payload},
    {"armor", "template.armor", lua_btech_template_armor},
    {"installed_parts", "template.installed_parts",
     lua_btech_template_installed_parts},
    {"critical_slots", "template.critical_slots",
     lua_btech_template_critical_slots},
    {"weapons", "template.weapons", lua_btech_template_weapons},
    {"technologies", "template.technologies", lua_btech_template_technologies},
    {"show_status", "template.show_status", lua_btech_template_show_status},
    {"show_weapon_specs", "template.show_weapon_specs",
     lua_btech_template_show_weapon_specs},
    {"show_critical_status", "template.show_critical_status",
     lua_btech_template_show_critical_status},
};

void lua_btech_install_template_bindings(lua_State *state,
                                         LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "template", BTECH_TEMPLATE_ENTRIES,
      sizeof(BTECH_TEMPLATE_ENTRIES) / sizeof(BTECH_TEMPLATE_ENTRIES[0]));
}
