#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/server/log.h"
#include "mux/world/database_check.h"

static bool checking;
static int database_check_calls;
static DatabaseCheckRequest database_check_request;
static bool write_result = true;
static char written_filename[64];
static char written_message[128];

void database_check(const DatabaseCheckRequest *request) {
  database_check_calls++;
  database_check_request = *request;
}

bool log_to_file(const ArbitraryLogRequest *request) {
  snprintf(written_filename, sizeof(written_filename), "%s", request->filename);
  snprintf(written_message, sizeof(written_message), "%s", request->message);
  return write_result;
}

bool object_state_transaction_begin(ObjectStateTransaction *transaction
                                    [[maybe_unused]],
                                    GameDatabase *database [[maybe_unused]]) {
  return true;
}

void object_state_transaction_finish(ObjectStateTransaction *transaction
                                     [[maybe_unused]],
                                     bool commit [[maybe_unused]]) {}

void object_state_transaction_initialize(ObjectStateTransaction *transaction) {
  memset(transaction, 0, sizeof(*transaction));
}

void object_state_transaction_destroy(ObjectStateTransaction *transaction
                                      [[maybe_unused]]) {}

void lua_mux_install_world_bindings(lua_State *state [[maybe_unused]],
                                    LuaMuxPackage *package [[maybe_unused]]) {}
void lua_mux_install_session_bindings(lua_State *state [[maybe_unused]],
                                      LuaMuxPackage *package [[maybe_unused]]) {
}
void lua_mux_install_text_bindings(lua_State *state [[maybe_unused]],
                                   LuaMuxPackage *package [[maybe_unused]]) {}
void lua_mux_install_telnet_bindings(lua_State *state [[maybe_unused]],
                                     LuaMuxPackage *package [[maybe_unused]]) {}
void lua_mux_install_config_bindings(lua_State *state [[maybe_unused]],
                                     LuaMuxPackage *package [[maybe_unused]]) {}

static int is_checking(void *context [[maybe_unused]]) { return checking; }

static bool run(lua_State *state, const char *script, bool expected) {
  if (luaL_loadstring(state, script) || lua_pcall(state, 0, 1, 0)) {
    lua_pop(state, 1);
    return false;
  }
  bool result = lua_toboolean(state, -1);
  lua_pop(state, 1);
  return result == expected;
}

static bool fails(lua_State *state, const char *script) {
  if (luaL_loadstring(state, script)) {
    lua_pop(state, 1);
    return false;
  }
  bool failed = lua_pcall(state, 0, 0, 0) != 0;
  if (failed)
    lua_pop(state, 1);
  return failed;
}

static bool check_db_returns_no_values(lua_State *state) {
  const int INITIAL_TOP = lua_gettop(state);

  lua_getglobal(state, "mux");
  lua_getfield(state, -1, "check_db");
  lua_remove(state, -2);
  if (lua_pcall(state, 0, LUA_MULTRET, 0)) {
    lua_settop(state, INITIAL_TOP);
    return false;
  }
  const bool NO_VALUES = lua_gettop(state) == INITIAL_TOP;
  lua_settop(state, INITIAL_TOP);
  return NO_VALUES;
}

int main(void) {
  lua_State *state = luaL_newstate();
  CommandContext background = {0};
  LuaServices services = {.background_command = &background};
  LuaMuxPackage package = {.services = &services, .is_checking = is_checking};

  if (state == nullptr)
    return 2;
  lua_mux_package_install(state, &package);
  if (!check_db_returns_no_values(state) || database_check_calls != 1 ||
      database_check_request.evaluation != &background.evaluation ||
      database_check_request.player != NOTHING ||
      database_check_request.options != 0) {
    lua_close(state);
    return 1;
  }
  if (!run(state, "return mux.log('combat.log', 'destroyed')", true) ||
      strcmp(written_filename, "combat.log") ||
      strcmp(written_message, "destroyed")) {
    lua_close(state);
    return 1;
  }
  write_result = false;
  if (!run(state, "return mux.log('combat.log', 'failed')", false)) {
    lua_close(state);
    return 1;
  }
  if (!fails(state, "mux.log('bad\\0name', 'message')") ||
      !fails(state, "mux.log('combat.log', 'bad\\0message')")) {
    lua_close(state);
    return 1;
  }
  checking = true;
  if (!fails(state, "mux.check_db()") || database_check_calls != 1 ||
      !fails(state, "mux.log('combat.log', 'checked')")) {
    lua_close(state);
    return 1;
  }
  lua_close(state);
  return 0;
}
