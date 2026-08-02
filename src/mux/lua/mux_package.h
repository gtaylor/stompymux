/* mux_package.h - Built-in Lua mux package. */

#pragma once

#include <lua.h>

#include "mux/objects/db.h"
#include "mux/objects/object_state.h"

typedef struct LuaMuxPackage LuaMuxPackage;
typedef struct LuaServices LuaServices;

typedef int (*LuaMuxPackageCheckingFn)(void *context);
typedef int (*LuaMuxPackageFlowStartFn)(void *context, lua_State *state,
                                        int descriptor_id, const char *module,
                                        const char *first_step);
typedef int (*LuaMuxPackageExitEnterLockPassesFn)(void *context, DbRef exit,
                                                  DbRef enactor);

struct LuaMuxPackage {
  /* Services and callback context are borrowed from the owning LuaRuntime. */
  const LuaServices *services;
  void *context;
  LuaMuxPackageCheckingFn is_checking;
  LuaMuxPackageFlowStartFn flow_start;
  LuaMuxPackageExitEnterLockPassesFn exit_enter_lock_passes;
  ObjectStateTransaction state_transaction;
};

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package);
bool lua_mux_package_transaction_begin(LuaMuxPackage *package);
void lua_mux_package_transaction_finish(LuaMuxPackage *package, bool commit);
void lua_mux_package_destroy(LuaMuxPackage *package);
