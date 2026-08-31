/* Private interfaces shared by built-in mux package binding modules. */

#pragma once

#include <stdint.h>

#include "mux/lua/packages/mux/mux_package.h"

extern const char LUA_MUX_OBJECT_METATABLE[];
extern const char LUA_MUX_STATE_METATABLE[];
extern const char LUA_MUX_ATTRIBUTE_METATABLE[];
extern const char LUA_MUX_FLAGS_METATABLE[];
extern const char LUA_MUX_POWERS_METATABLE[];
extern const char LUA_MUX_FLAG_METATABLE[];
extern const char LUA_MUX_POWER_METATABLE[];

typedef struct LuaMuxObject LuaMuxObject;
struct LuaMuxObject {
  LuaMuxPackage *package;
  DbRef object;
  uint64_t generation;
};

typedef struct LuaMuxState LuaMuxState;
struct LuaMuxState {
  LuaMuxPackage *package;
  DbRef object;
  uint64_t generation;
  char name_space[128];
};

typedef struct LuaMuxAttribute LuaMuxAttribute;
struct LuaMuxAttribute {
  LuaMuxPackage *package;
  DbRef object;
  uint64_t generation;
};

typedef struct LuaMuxObjectSet LuaMuxObjectSet;
struct LuaMuxObjectSet {
  LuaMuxPackage *package;
  DbRef object;
  uint64_t generation;
};

typedef struct LuaMuxNamedConstant LuaMuxNamedConstant;
struct LuaMuxNamedConstant {
  LuaMuxPackage *package;
  int id;
  const char *name;
};

LuaMuxPackage *lua_mux_package_get(lua_State *state);
bool lua_mux_package_is_checking(LuaMuxPackage *package);
void lua_mux_require_runtime(LuaMuxPackage *package, lua_State *state,
                             const char *function);
void lua_mux_check_options(lua_State *state, int table,
                           const char *const *allowed, size_t allowed_count);
DbRef lua_mux_option_object(LuaMuxPackage *package, lua_State *state, int table,
                            const char *field, bool required, bool *present);
DbRef lua_mux_require_object(LuaMuxPackage *package, lua_State *state,
                             int argument);
DbRef lua_mux_require_object_at(LuaMuxPackage *package, lua_State *state,
                                int index, int argument, const char *label);
LuaMuxObject *lua_mux_push_object(lua_State *state, LuaMuxPackage *package,
                                  DbRef object);
LuaMuxObject *lua_mux_check_object_handle(lua_State *state, int argument);
LuaMuxState *lua_mux_check_state(lua_State *state, int argument);
LuaMuxAttribute *lua_mux_check_attribute(lua_State *state, int argument);

void lua_mux_install_object_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_object_relationship_bindings(lua_State *state,
                                                  LuaMuxPackage *package);
void lua_mux_install_state_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_attribute_bindings(lua_State *state,
                                        LuaMuxPackage *package);
void lua_mux_install_flag_power_bindings(lua_State *state,
                                         LuaMuxPackage *package);
/** Installs typed lock constants and evaluation bindings.
 * @param[in,out] state Lua state whose top value is the world table.
 * @param[in,out] package Owning package. */
void lua_mux_install_lock_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_text_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_world_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_session_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_telnet_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_error_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_config_bindings(lua_State *state, LuaMuxPackage *package);
