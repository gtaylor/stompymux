/* btech_repair_bindings.c - Native Lua bindings for btech.repair. */

#include <lua.h>
#include <stddef.h>
#include <time.h>

#include "btech/configuration.h"
#include "btech/repair/mech_tech_api.h"
#include "btech/repair/mech_tech_commands_api.h"
#include "btech/repair/mech_tech_damages.h"
#include "btech/repair/mech_tech_damages_api.h"
#include "btech/special/registry_api.h"
#include "btech/special_objects.h"
#include "btech/unit/mech_classification_api.h"
#include "btech/unit/mech_specification_api.h"
#include "btech/unit/mech_utils_api.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/btech/btech_package.h"
#include "mux/lua/packages/btech/btech_package_internal.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h"
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

static const char *operation_name(int operation) {
  static const char *const NAMES[] = {
      "reattach",
      "repair_part",
      "repair_weapon_temporary",
      "repair_enhancement",
      "repair_focus",
      "repair_crystal",
      "repair_barrel",
      "repair_ammo_feed",
      "repair_ranging",
      "repair_ammo_mount",
      "replace_weapon",
      "reload",
      "repair_armor",
      "repair_rear_armor",
      "repair_internal",
      "detach",
      "scrap_part",
      "scrap_weapon",
      "unload",
      "reseal",
      "replace_suit",
  };
  if (operation < 0 || operation >= (int)(sizeof(NAMES) / sizeof(NAMES[0])))
    return "unknown";
  return *(const char *const *)checked_storage_at_const(
      (const void *)NAMES, sizeof(NAMES) / sizeof(NAMES[0]), sizeof(NAMES[0]),
      (size_t)operation);
}

typedef struct RepairPushContext {
  lua_State *state;
  Mech *mech;
  int index;
} RepairPushContext;

static bool push_need(const BtechRepairNeed *need, void *opaque) {
  RepairPushContext *context = opaque;
  lua_State *state = context->state;
  const UnitSectionCatalog CATALOG = {
      .unit_type = mech_class(context->mech),
      .movement_type = mech_movement_type(context->mech),
  };
  lua_newtable(state);
  lua_pushstring(state, operation_name(need->operation));
  lua_setfield(state, -2, "operation");
  const char *section = unit_section_name(&CATALOG, (size_t)need->section);
  if (section == nullptr)
    (void)lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                          "repair need contains an invalid section");
  lua_pushstring(state, section);
  lua_setfield(state, -2, "section");
  lua_pushboolean(state, (int)need->in_progress);
  lua_setfield(state, -2, "in_progress");
  if (need->operation == FIXARMOR || need->operation == FIXARMOR_R ||
      need->operation == FIXINTERNAL) {
    lua_pushinteger(state, need->detail);
    lua_setfield(state, -2, "amount");
  } else if (need->detail >= 0 && need->operation != REATTACH &&
             need->operation != DETACH && need->operation != RESEAL &&
             need->operation != REPLACESUIT) {
    lua_pushinteger(state, need->detail + 1);
    lua_setfield(state, -2, "slot");
  }
  lua_rawseti(state, -2, context->index++);
  return true;
}

static int lua_btech_repair_needs(lua_State *state, LuaBtechPackage *package) {
  Mech *mech = require_mech(state, package);
  lua_newtable(state);
  RepairPushContext context = {.state = state, .mech = mech, .index = 1};
  btech_repair_needs_visit(mech, push_need, &context);
  return 1;
}

static int lua_btech_repair_is_under_repair(lua_State *state,
                                            LuaBtechPackage *package) {
  lua_pushboolean(state,
                  figure_latest_tech_event(require_mech(state, package)) > 0);
  return 1;
}

static int lua_btech_repair_is_fixable(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_pushboolean(state, unit_is_fixable(require_mech(state, package)) ? 1 : 0);
  return 1;
}

static int lua_btech_repair_technician_available_in(lua_State *state,
                                                    LuaBtechPackage *package) {
  const DbRef PLAYER = lua_btech_require_object(package, state, 1);
  if (!is_player(package->services->database, PLAYER))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not a player");
  const time_t AVAILABLE =
      btech_repair_technician_available_at(lua_btech_context(package), PLAYER);
  const time_t NOW = package->services->clock->now;
  lua_pushinteger(state, AVAILABLE > NOW ? (lua_Integer)(AVAILABLE - NOW) : 0);
  return 1;
}

static const BtechLuaNativeEntry BTECH_REPAIR_ENTRIES[] = {
    {"needs", "repair.needs", lua_btech_repair_needs},
    {"is_under_repair", "repair.is_under_repair",
     lua_btech_repair_is_under_repair},
    {"is_fixable", "repair.is_fixable", lua_btech_repair_is_fixable},
    {"technician_available_in", "repair.technician_available_in",
     lua_btech_repair_technician_available_in},
};

void lua_btech_install_repair_bindings(lua_State *state,
                                       LuaBtechPackage *package) {
  lua_btech_install_native_bindings(
      state, package, "repair", BTECH_REPAIR_ENTRIES,
      sizeof(BTECH_REPAIR_ENTRIES) / sizeof(BTECH_REPAIR_ENTRIES[0]));
}
