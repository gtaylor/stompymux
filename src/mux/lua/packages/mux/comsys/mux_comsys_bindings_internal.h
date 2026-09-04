/* Private definitions shared by mux.comsys binding modules. */

#pragma once

#include <lua.h>
#include <stdint.h>

#include "mux/communication/comsys.h"
#include "mux/lua/packages/mux/mux_package.h"

typedef struct LuaMuxChannel LuaMuxChannel;
struct LuaMuxChannel {
  LuaMuxPackage *package;
  struct Channel *identity;
  uint64_t generation;
  char name[CHAN_NAME_LEN];
};

extern const char LUA_MUX_CHANNEL_METATABLE[];

LuaMuxChannel *lua_mux_check_channel(lua_State *state, int argument);
struct Channel *lua_mux_check_channel_identity(LuaMuxPackage *package,
                                               lua_State *state, int argument,
                                               const char *name,
                                               struct Channel *identity,
                                               uint64_t generation);
int lua_mux_channel_add_player(lua_State *state);
void lua_mux_install_channel_flag_bindings(lua_State *state,
                                           LuaMuxPackage *package);
