/* lua_error.c -- Structured Lua error tests. */

#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/packages/mux/mux_package_internal.h"

static bool run(lua_State *state, const char *source) {
  return luaL_loadstring(state, source) == 0 && lua_pcall(state, 0, 1, 0) == 0;
}

int main(void) {
  lua_State *state = luaL_newstate();
  char description[32];

  if (!state)
    return 2;
  luaL_openlibs(state);
  lua_error_install(state);
  lua_newtable(state);
  lua_mux_install_error_bindings(state, nullptr);
  lua_setglobal(state, "mux");
  if (!run(state, "local codes = mux.error.codes "
                  "return codes == mux.error.code_tree('mux') and "
                  "codes.code == 'mux' and codes.state == codes.state and "
                  "codes.state.code == 'mux.state' and "
                  "codes.state.value_too_large.code == "
                  "'mux.state.value_too_large'"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state, "local found = nil "
                  "for key, value in pairs(mux.error.codes.state) do "
                  "if key == 'value_too_large' then found = value.code end end "
                  "return found == 'mux.state.value_too_large'"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state, "local codes = mux.error.codes "
                  "local ok = pcall(function() return codes.stat end) "
                  "return not ok"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state, "local codes = mux.error.codes "
                  "local ok = pcall(function() codes.state.extra = true end) "
                  "return not ok"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state,
           "local codes = mux.error.codes "
           "local err = mux.error.new({ "
           "code = codes.state.value_too_large, message = 'too large' }) "
           "local ok, raised = pcall(mux.error.raise, "
           "codes.state.value_too_large, 'too large') "
           "return err.code == 'mux.state.value_too_large' and "
           "err:is(codes.state) and mux.error.is(err, codes.state) "
           "and not ok and raised.code == 'mux.state.value_too_large'"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state,
           "local caught = { code = 'author.caught', token = 17 } "
           "local ok, err = mux.error.pcall(function() error(caught) end) "
           "return not ok and err == caught and err.code == 'author.caught' "
           "and err.token == 17 and err.message == nil "
           "and type(err.traceback) == 'string'"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state,
           "local code = 'NOT valid..code!' "
           "local made = mux.error.new({ code = code, message = 'made' }) "
           "local raised_ok, raised = pcall(mux.error.raise, code, 'raised') "
           "local wrapped = mux.error.wrap('cause', code, 'wrapped') "
           "return made.code == code and made:is(code) "
           "and mux.error.is(made, code) and not raised_ok "
           "and raised.code == code and wrapped.code == code"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state,
           "local btech = mux.error.code_tree('btech') "
           "local testing = mux.error.code_tree('testing') "
           "return btech == mux.error.code_tree('btech') and "
           "btech.code == 'btech' and btech.unavailable.code == "
           "'btech.unavailable' and testing == mux.error.code_tree('testing') "
           "and testing.code == 'testing' and "
           "testing.assertion.code == 'testing.assertion' and "
           "mux.error.new({ code = btech.failed, message = 'failed' "
           "}):is(btech)"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state, "return not pcall(mux.error.code_tree, 'unknown')"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state,
           "local codes = mux.error.namespace('cargo', "
           "{ 'full', 'bay.full', 'no_capacity' }) "
           "return codes.code == 'cargo' and codes.full.code == 'cargo.full' "
           "and codes.bay.code == 'cargo.bay' and "
           "codes.bay.full.code == 'cargo.bay.full' and "
           "tostring(codes.no_capacity) == 'cargo.no_capacity'"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state, "local roots = { 'mux', 'btech', 'testing' } "
                  "for _, root in ipairs(roots) do "
                  "if pcall(mux.error.namespace, root, { 'invalid' }) then "
                  "return false end end return true"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  lua_error_push(state, "mux.state.value_too_large",
                 "state value exceeds 4096 bytes");
  lua_setglobal(state, "err");
  if (!run(state, "return tostring(err) == 'mux.state.value_too_large: state "
                  "value exceeds 4096 bytes' and err:is('mux.state')"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  lua_error_push(state, "testing.assertion", "inner");
  lua_setglobal(state, "inner");
  if (!run(state, "inner.cause = inner; return inner:root() == inner"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  if (!run(state, "err.cause = inner; return err:root() == inner"))
    goto failed;
  if (!lua_toboolean(state, -1))
    goto failed;
  lua_pop(state, 1);
  lua_getglobal(state, "err");
  lua_error_describe(state, -1, description, sizeof(description));
  if (strcmp(description, "mux.state.value_too_large: stat"))
    goto failed;
  lua_pop(state, 1);
  lua_pushinteger(state, 42);
  lua_error_describe(state, -1, description, sizeof(description));
  if (strcmp(description, "42"))
    goto failed;
  lua_close(state);
  return 0;
failed:
  lua_close(state);
  return 1;
}
