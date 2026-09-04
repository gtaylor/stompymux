/** @file
 * Built-in Lua BattleTech package.
 */
#pragma once

#include <lua.h>

#include "mux/lua/lua_runtime.h"

typedef struct LuaBtechPackage LuaBtechPackage;
typedef struct LuaMuxPackage LuaMuxPackage;
typedef struct LuaServices LuaServices;

struct LuaBtechPackage {
  const LuaServices *services;
  LuaMuxPackage *mux_package;
  void *context;
  int (*is_checking)(void *context);
};

/** Executes lua btech package install. @param[in,out] state State to inspect or
 * update. @param[in,out] package Package. */

void lua_btech_package_install(lua_State *state, LuaBtechPackage *package);
