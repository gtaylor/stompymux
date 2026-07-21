/* btech_package.h - Built-in Lua BattleTech package. */

#pragma once

#include <lua.h>

typedef struct LuaBtechPackage LuaBtechPackage;
typedef struct LuaServices LuaServices;

struct LuaBtechPackage {
  const LuaServices *services;
  void *context;
  int (*is_checking)(void *context);
};

void lua_btech_package_install(lua_State *state, LuaBtechPackage *package);
