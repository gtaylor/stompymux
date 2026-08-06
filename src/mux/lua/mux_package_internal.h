/* Private interfaces shared by built-in mux package binding modules. */

#pragma once

#include <stdint.h>

#include "mux/lua/mux_package.h"

extern const char LUA_MUX_OBJECT_METATABLE[];
extern const char LUA_MUX_STATE_METATABLE[];
extern const char LUA_MUX_ATTRIBUTE_METATABLE[];

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

LuaMuxPackage *lua_mux_package_get(lua_State *state);
int lua_mux_package_is_checking(LuaMuxPackage *package);
void lua_mux_require_runtime(LuaMuxPackage *package, lua_State *state,
                             const char *function);
DbRef lua_mux_require_object(LuaMuxPackage *package, lua_State *state,
                             int argument);
LuaMuxObject *lua_mux_push_object(lua_State *state, LuaMuxPackage *package,
                                  DbRef object);
LuaMuxObject *lua_mux_check_object_handle(lua_State *state, int argument);
LuaMuxState *lua_mux_check_state(lua_State *state, int argument);
LuaMuxAttribute *lua_mux_check_attribute(lua_State *state, int argument);

void lua_mux_install_object_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_state_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_attribute_bindings(lua_State *state,
                                        LuaMuxPackage *package);
void lua_mux_install_text_bindings(lua_State *state, LuaMuxPackage *package);
void lua_mux_install_connection_bindings(lua_State *state,
                                         LuaMuxPackage *package);
