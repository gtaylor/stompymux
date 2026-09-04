/* Private interfaces shared by built-in btech package binding modules. */

#pragma once

#include <stddef.h>

#include "btech/unit/mech_partnames_api.h"
#include "mux/lua/packages/btech/btech_package.h"

typedef struct Mech Mech;
typedef int BtechLuaNativeFunction(lua_State *state, LuaBtechPackage *package);
typedef struct BtechLuaNativeEntry BtechLuaNativeEntry;
struct BtechLuaNativeEntry {
  const char *name;
  const char *qualified_name;
  BtechLuaNativeFunction *handler;
};

void lua_btech_install_native_bindings(lua_State *state,
                                       LuaBtechPackage *package,
                                       const char *name,
                                       const BtechLuaNativeEntry *entries,
                                       size_t entry_count);
void lua_btech_check_arity(lua_State *state, int expected);
/** Raises `btech.operation.failed` with a stable `detail.reason`. */
int lua_btech_operation_error(lua_State *state, const char *reason,
                              const char *message);
void lua_btech_check_options(lua_State *state, int table,
                             const char *const *allowed, size_t allowed_count,
                             int argument);
void lua_btech_get_field(lua_State *state, int table, const char *field);
long lua_btech_check_integer_field(lua_State *state, int table,
                                   const char *field, long minimum,
                                   long maximum, int argument);
bool lua_btech_check_boolean_field(lua_State *state, int table,
                                   const char *field, int argument);
const char *lua_btech_check_string_field(lua_State *state, int table,
                                         const char *field, size_t maximum,
                                         int argument);
DbRef lua_btech_require_object(LuaBtechPackage *package, lua_State *state,
                               int argument);
DbRef lua_btech_require_object_field(LuaBtechPackage *package, lua_State *state,
                                     int table, const char *field,
                                     int argument);
DbRef lua_btech_require_special(LuaBtechPackage *package, lua_State *state,
                                int argument, int type, const char *label);
void lua_btech_push_object(lua_State *state, LuaBtechPackage *package,
                           DbRef object);
void lua_btech_push_optional_object(lua_State *state, LuaBtechPackage *package,
                                    DbRef object);
struct BtechContext *lua_btech_context(LuaBtechPackage *package);
/** Rejects path traversal syntax in a script-supplied resource name. */
void lua_btech_validate_resource_name(lua_State *state, int argument,
                                      const char *name, const char *label);
bool lua_btech_check_part(lua_State *state, BtechContext *context, int index,
                          int argument, PartReference *part);
void lua_btech_push_part(lua_State *state, BtechContext *context,
                         PartReference part);
void lua_btech_push_critical_modes(lua_State *state, unsigned int modes,
                                   bool ammunition);
void lua_btech_push_payload(lua_State *state, BtechContext *context,
                            Mech *mech);
void lua_btech_push_installed_parts(lua_State *state, BtechContext *context,
                                    Mech *mech);
void lua_btech_push_armor(lua_State *state, Mech *mech, int section);
int lua_btech_optional_section(lua_State *state, Mech *mech, int argument);
void lua_btech_push_critical_slots(lua_State *state, BtechContext *context,
                                   Mech *mech, int section);
void lua_btech_push_weapons(lua_State *state, BtechContext *context, Mech *mech,
                            int section);
void lua_btech_push_technologies(lua_State *state, Mech *mech);
void lua_btech_push_battle_value(lua_State *state, Mech *mech);
void lua_btech_install_unit_bindings(lua_State *state,
                                     LuaBtechPackage *package);
void lua_btech_install_unit_operation_bindings(lua_State *state,
                                               LuaBtechPackage *package);
void lua_btech_install_map_bindings(lua_State *state, LuaBtechPackage *package);
void lua_btech_install_map_los_bindings(lua_State *state,
                                        LuaBtechPackage *package);
void lua_btech_install_player_bindings(lua_State *state,
                                       LuaBtechPackage *package);
void lua_btech_install_parts_bindings(lua_State *state,
                                      LuaBtechPackage *package);
void lua_btech_install_character_bindings(lua_State *state,
                                          LuaBtechPackage *package);
void lua_btech_install_repair_bindings(lua_State *state,
                                       LuaBtechPackage *package);
void lua_btech_install_system_bindings(lua_State *state,
                                       LuaBtechPackage *package);
void lua_btech_install_template_bindings(lua_State *state,
                                         LuaBtechPackage *package);
