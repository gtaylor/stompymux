/* mux_package.h - Built-in Lua mux package. */

#pragma once

#include <lua.h>

typedef struct LuaMuxPackage LuaMuxPackage;

typedef int (*LuaMuxPackageCheckingFn)(void *context);
typedef int (*LuaMuxPackageFlowStartFn)(void *context, lua_State *state,
                                        int descriptor_id, const char *module,
                                        const char *first_step);

struct LuaMuxPackage {
  void *context;
  LuaMuxPackageCheckingFn is_checking;
  LuaMuxPackageFlowStartFn flow_start;
};

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package);
