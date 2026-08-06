/* mux_package.c - Built-in Lua mux package bindings. */

#include <lauxlib.h>

#include "mux/lua/mux_package.h"
#include "mux/lua/mux_package_internal.h"
#include "mux/objects/object_state.h"

LuaMuxPackage *lua_mux_package_get(lua_State *state) {
  return lua_touserdata(state, lua_upvalueindex(1));
}

const char LUA_MUX_OBJECT_METATABLE[] = "btmux.object";
const char LUA_MUX_STATE_METATABLE[] = "btmux.object_state";
const char LUA_MUX_ATTRIBUTE_METATABLE[] = "btmux.object_attribute";

bool lua_mux_package_transaction_begin(LuaMuxPackage *package) {
  return object_state_transaction_begin(&package->state_transaction,
                                        package->services->database);
}

void lua_mux_package_transaction_finish(LuaMuxPackage *package, bool commit) {
  object_state_transaction_finish(&package->state_transaction, commit);
}

void lua_mux_package_destroy(LuaMuxPackage *package) {
  object_state_transaction_destroy(&package->state_transaction);
}

int lua_mux_package_is_checking(LuaMuxPackage *package) {
  return package->is_checking && package->is_checking(package->context);
}

void lua_mux_require_runtime(LuaMuxPackage *package, lua_State *state,
                             const char *function) {
  if (lua_mux_package_is_checking(package))
    luaL_error(state, "mux.%s is unavailable during @lua/check", function);
}

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package) {
  object_state_transaction_initialize(&package->state_transaction);
  lua_newtable(state);
  lua_mux_install_object_bindings(state, package);
  lua_mux_install_state_bindings(state, package);
  lua_mux_install_attribute_bindings(state, package);
  lua_mux_install_text_bindings(state, package);
  lua_mux_install_connection_bindings(state, package);
  lua_setglobal(state, "mux");
}
