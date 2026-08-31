/* lua_lock_checks.c -- Lua world lock bindings */

#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>

#include <stdio.h>
#include <string.h>

#include "mux/commands/command_context.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/lua_lock_catalog.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/support/checked_storage.h"

typedef struct LockRecorder {
  LuaLockInvocation invocation;
  int calls;
  bool checking;
  bool passes;
} LockRecorder;

typedef struct ExpectedLock {
  const char *key;
  const char *name;
  LuaLockType type;
} ExpectedLock;

static const ExpectedLock EXPECTED_LOCKS[] = {
    {"match", "MATCH", LUA_LOCK_MATCH},
    {"traverse", "TRAVERSE", LUA_LOCK_TRAVERSE},
    {"take", "TAKE", LUA_LOCK_TAKE},
    {"use", "USE", LUA_LOCK_USE},
    {"drop", "DROP", LUA_LOCK_DROP},
    {"give", "GIVE", LUA_LOCK_GIVE},
    {"receive", "RECEIVE", LUA_LOCK_RECEIVE},
    {"enter", "ENTER", LUA_LOCK_ENTER},
    {"leave", "LEAVE", LUA_LOCK_LEAVE},
    {"teleport", "TELEPORT", LUA_LOCK_TELEPORT},
    {"teleport_out", "TELEPORT_OUT", LUA_LOCK_TELEPORT_OUT},
    {"link", "LINK", LUA_LOCK_LINK},
    {"set_home", "SET_HOME", LUA_LOCK_SET_HOME},
    {"speak", "SPEAK", LUA_LOCK_SPEAK},
    {"channel_join", "CHANNEL_JOIN", LUA_LOCK_CHANNEL_JOIN},
    {"channel_transmit", "CHANNEL_TRANSMIT", LUA_LOCK_CHANNEL_TRANSMIT},
    {"channel_receive", "CHANNEL_RECEIVE", LUA_LOCK_CHANNEL_RECEIVE},
    {"identify_building", "IDENTIFY_BUILDING", LUA_LOCK_IDENTIFY_BUILDING},
};

LuaMuxPackage *lua_mux_package_get(lua_State *state) {
  return lua_touserdata(state, lua_upvalueindex(1));
}

bool lua_mux_package_is_checking(LuaMuxPackage *package) {
  LockRecorder *recorder = package->context;

  return recorder->checking;
}

void lua_mux_require_runtime(LuaMuxPackage *package, lua_State *state,
                             const char *function) {
  if (lua_mux_package_is_checking(package))
    lua_error_raise(state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
                    "mux.%s is unavailable during @lua/check", function);
}

DbRef lua_mux_require_object(LuaMuxPackage *package [[maybe_unused]],
                             lua_State *state, int argument) {
  return lua_mux_require_object_at(package, state, argument, argument,
                                   "object");
}

DbRef lua_mux_require_object_at(LuaMuxPackage *package [[maybe_unused]],
                                lua_State *state, int index, int argument,
                                const char *label) {
  if (!lua_isnumber(state, index))
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "%s must be a dbref or Object", label);
  DbRef object = (DbRef)lua_tointeger(state, index);

  if (object < 0 || object > 100)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "%s is invalid", label);
  return object;
}

static bool record_lock(void *context, const LuaLockInvocation *invocation) {
  LockRecorder *recorder = context;

  recorder->invocation = *invocation;
  recorder->calls++;
  return recorder->passes;
}

static void install_world(lua_State *state, LuaMuxPackage *package,
                          const char *global) {
  lua_newtable(state);
  lua_mux_install_lock_bindings(state, package);
  lua_setglobal(state, global);
}

static bool run_boolean(lua_State *state, const char *script) {
  if (luaL_loadstring(state, script) || lua_pcall(state, 0, 1, 0)) {
    lua_pop(state, 1);
    return false;
  }
  bool result = lua_toboolean(state, -1) != 0;

  lua_pop(state, 1);
  return result;
}

static bool raises_code(lua_State *state, const char *script,
                        const char *expected) {
  if (luaL_loadstring(state, script)) {
    lua_pop(state, 1);
    return false;
  }
  int status = lua_pcall(state, 0, 0, 0);
  const char *code = status ? lua_error_check_code(state, -1) : nullptr;
  bool matches = code && !strcmp(code, expected);

  if (status)
    lua_pop(state, 1);
  return status != 0 && matches;
}

static bool test_all_locks(lua_State *state, LockRecorder *recorder) {
  char script[256];

  for (size_t index = 0;
       index < sizeof(EXPECTED_LOCKS) / sizeof(*EXPECTED_LOCKS); index++) {
    const ExpectedLock *expected = checked_storage_at_const(
        EXPECTED_LOCKS, sizeof(EXPECTED_LOCKS) / sizeof(*EXPECTED_LOCKS),
        sizeof(*EXPECTED_LOCKS), index);
    const LuaLockDefinition *definition = lua_lock_definition_at(index);

    if (definition->type != expected->type ||
        strcmp(definition->key, expected->key) ||
        strcmp(definition->constant, expected->name) ||
        lua_lock_definition_find_key(expected->key) != definition ||
        lua_lock_definition_find_constant(expected->name) != definition ||
        snprintf(script, sizeof(script),
                 "return world.lock_passes({object=1,enactor=2,lock=world."
                 "locks.%s})",
                 expected->name) >= (int)sizeof(script) ||
        !run_boolean(state, script) ||
        recorder->invocation.type != expected->type ||
        recorder->invocation.object != 1 || recorder->invocation.enactor != 2 ||
        recorder->invocation.cause != 2 || recorder->invocation.subject != 2 ||
        recorder->invocation.descriptor != (Descriptor *)(void *)recorder ||
        !recorder->invocation.silent)
      return false;
  }
  return recorder->calls ==
             (int)(sizeof(EXPECTED_LOCKS) / sizeof(*EXPECTED_LOCKS)) &&
         sizeof(EXPECTED_LOCKS) / sizeof(*EXPECTED_LOCKS) == LUA_LOCK_COUNT;
}

static bool test_context_overrides(lua_State *state, LockRecorder *recorder) {
  if (!run_boolean(
          state,
          "return world.lock_passes({object=10,enactor=20,cause=30,subject=40,"
          "lock=world.locks.TRAVERSE})"))
    return false;
  return recorder->invocation.object == 10 &&
         recorder->invocation.enactor == 20 &&
         recorder->invocation.cause == 30 &&
         recorder->invocation.subject == 40 && recorder->invocation.silent;
}

static bool test_failed_check(lua_State *state, LockRecorder *recorder) {
  recorder->passes = false;
  bool failed = run_boolean(
      state,
      "return not world.lock_passes({object=1,enactor=2,lock=world.locks."
      "USE})");

  recorder->passes = true;
  return failed;
}

static bool test_constant_behavior(lua_State *state) {
  return run_boolean(state, "local a=world.locks.TRAVERSE local "
                            "b=world.locks.TRAVERSE return a==b and "
                            "tostring(a)=='TRAVERSE'") &&
         raises_code(state, "return world.locks.UNKNOWN", "mux.arg.invalid") &&
         raises_code(state, "return world.locks.DEFAULT", "mux.arg.invalid") &&
         raises_code(state, "return world.locks.SPEECH", "mux.arg.invalid") &&
         raises_code(state, "return world.locks.CONTACT", "mux.arg.invalid") &&
         raises_code(state, "world.locks.TRAVERSE.value=true",
                     "mux.arg.invalid") &&
         raises_code(state, "world.locks.TRAVERSE=true", "mux.arg.invalid");
}

static bool test_validation(lua_State *state, LuaMuxPackage *package,
                            LockRecorder *recorder) {
  if (!raises_code(state, "return world.lock_passes('bad')",
                   "mux.arg.invalid") ||
      !raises_code(
          state,
          "return world.lock_passes({object=1,enactor=2,lock='traverse'})",
          "mux.arg.invalid") ||
      !raises_code(
          state,
          "return world.lock_passes({object=1,enactor=2,check=world.locks."
          "TRAVERSE})",
          "mux.arg.invalid") ||
      !raises_code(
          state,
          "return world.lock_passes({object=1,enactor=2,lock=world.locks."
          "TRAVERSE,extra=true})",
          "mux.arg.invalid") ||
      !raises_code(
          state,
          "return world.lock_passes({enactor=2,lock=world.locks.TRAVERSE})",
          "mux.arg.invalid") ||
      !raises_code(
          state,
          "return world.lock_passes({object=999,enactor=2,lock=world.locks."
          "TRAVERSE})",
          "mux.object.invalid") ||
      !run_boolean(
          state,
          "local ok,err=pcall(function() return world.lock_passes({object=1,"
          "enactor=2,cause='bad',lock=world.locks.TRAVERSE}) end) return not "
          "ok and err.detail.argument==1 and "
          "err.message:find('options.cause',1,true)~=nil") ||
      !raises_code(
          state,
          "return world.lock_passes({object=1,enactor=2,lock=other.locks."
          "TRAVERSE})",
          "mux.arg.invalid"))
    return false;

  recorder->checking = true;
  bool checking = raises_code(
      state,
      "return world.lock_passes({object=1,enactor=2,lock=world.locks."
      "TRAVERSE})",
      "mux.unavailable.checking");
  recorder->checking = false;
  LuaMuxPackageLockPassesFn callback = package->lock_passes;

  package->lock_passes = nullptr;
  bool unavailable = raises_code(
      state,
      "return world.lock_passes({object=1,enactor=2,lock=world.locks."
      "TRAVERSE})",
      "mux.object.unavailable");
  package->lock_passes = callback;
  return checking && unavailable;
}

int main(void) {
  lua_State *state = luaL_newstate();
  LockRecorder recorder = {.passes = true};
  LockRecorder other_recorder = {.passes = true};
  CommandContext command = {.descriptor = (Descriptor *)(void *)&recorder};
  LuaServices services = {.background_command = &command};
  LuaMuxPackage package = {
      .services = &services, .context = &recorder, .lock_passes = record_lock};
  LuaMuxPackage other_package = {.services = &services,
                                 .context = &other_recorder,
                                 .lock_passes = record_lock};

  if (!state)
    return 2;
  luaL_openlibs(state);
  install_world(state, &package, "world");
  install_world(state, &other_package, "other");

  bool passes = test_all_locks(state, &recorder) &&
                test_context_overrides(state, &recorder) &&
                test_failed_check(state, &recorder) &&
                test_constant_behavior(state) &&
                test_validation(state, &package, &recorder);

  lua_close(state);
  return passes ? 0 : 1;
}
