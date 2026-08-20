#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* mux_connection_bindings.c - Lua bindings for connections and flows. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/network/descriptor.h"
#include "mux/network/telnet_environment.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h" // IWYU pragma: keep
#include "mux/support/utf8.h"

static int lua_mux_notify(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  size_t length;
  const char *message = luaL_checklstring(state, 2, &length);

  if (lua_mux_package_is_checking(package))
    return lua_error_raise(state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
                           "mux.notify is unavailable during @lua/check");
  if (strlen(message) != length)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CONNECTION_INVALID,
                         "message contains an embedded NUL byte");
  if (!utf8_validate(message, length))
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CONNECTION_INVALID,
                         "message is not valid UTF-8");
  object = lua_mux_require_object(package, state, 1);
  notify_checked(&package->services->background_command->evaluation, object,
                 object, message, MSG_ME_ALL | MSG_F_DOWN);
  return 0;
}

static int lua_mux_connected_players(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor;
  DescriptorIterator iterator =
      descriptor_iterator_connected(package->services->descriptors);
  int index = 1;

  lua_newtable(state);
  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    lua_newtable(state);
    lua_mux_push_object(state, package, descriptor->player);
    lua_setfield(state, -2, "object");
    lua_pushstring(state, game_object_name(package->services->database,
                                           descriptor->player));
    lua_setfield(state, -2, "name");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->connected_at));
    lua_setfield(state, -2, "connected_for");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->last_time));
    lua_setfield(state, -2, "idle_for");
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

static int lua_mux_who_summary(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_newtable(state);
  lua_pushinteger(state, 0);
  lua_setfield(state, -2, "hidden");
  lua_pushinteger(state, *package->services->record_players);
  lua_setfield(state, -2, "record");
  if (package->services->configuration->max_players == -1)
    lua_pushnil(state);
  else
    lua_pushinteger(state, package->services->configuration->max_players);
  lua_setfield(state, -2, "maximum");
  return 1;
}

static Descriptor *lua_mux_require_descriptor(LuaMuxPackage *package,
                                              lua_State *state, int argument) {
  int descriptor_id = (int)luaL_checkinteger(state, argument);
  Descriptor *descriptor;

  lua_mux_require_runtime(package, state, "telnet_environment");
  descriptor =
      descriptor_find_by_fd(package->services->descriptors, descriptor_id);
  if (descriptor == nullptr)
    lua_error_arg(state, argument, LUA_ERROR_CODE_CONNECTION_INVALID,
                  "no such descriptor");
  return descriptor;
}

static TelnetEnvironmentKind lua_mux_telnet_environment_kind(lua_State *state,
                                                             int argument) {
  const char *kind = luaL_checkstring(state, argument);

  if (!strcmp(kind, "var"))
    return TELNET_ENVIRONMENT_VAR;
  if (!strcmp(kind, "uservar"))
    return TELNET_ENVIRONMENT_USERVAR;
  lua_error_arg(state, argument, LUA_ERROR_CODE_CONNECTION_INVALID,
                "kind must be 'var' or 'uservar'");
  return TELNET_ENVIRONMENT_VAR;
}

static int lua_mux_telnet_environment_has(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor = lua_mux_require_descriptor(package, state, 1);
  TelnetEnvironmentKind kind = lua_mux_telnet_environment_kind(state, 2);
  size_t name_size;
  const char *name = luaL_checklstring(state, 3, &name_size);

  lua_pushboolean(state, descriptor_telnet_environment_has(descriptor, kind,
                                                           name, name_size));
  return 1;
}

static int lua_mux_telnet_environment_get(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor = lua_mux_require_descriptor(package, state, 1);
  TelnetEnvironmentKind kind = lua_mux_telnet_environment_kind(state, 2);
  size_t name_size;
  const char *name = luaL_checklstring(state, 3, &name_size);
  const void *value;
  size_t value_size;

  if (descriptor_telnet_environment_get(descriptor, kind, name, name_size,
                                        &value, &value_size))
    lua_pushlstring(state, value, value_size);
  else
    lua_pushnil(state);
  return 1;
}

static int lua_mux_flow_start(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  int descriptor_id = (int)luaL_checkinteger(state, 1);
  const char *module = luaL_checkstring(state, 2);
  const char *first_step = luaL_checkstring(state, 3);

  if (lua_mux_package_is_checking(package))
    return lua_error_raise(state, LUA_ERROR_CODE_CHECKING_UNAVAILABLE,
                           "mux.flow_start is unavailable during @lua/check");
  if (!package->flow_start)
    return lua_error_raise(state, LUA_ERROR_CODE_CONNECTION_UNAVAILABLE,
                           "mux.flow_start is unavailable");
  return package->flow_start(package->context, state, descriptor_id, module,
                             first_step);
}

void lua_mux_install_connection_bindings(lua_State *state,
                                         LuaMuxPackage *package) {
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_notify, 1);
  lua_setfield(state, -2, "notify");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_connected_players, 1);
  lua_setfield(state, -2, "connected_players");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_who_summary, 1);
  lua_setfield(state, -2, "who_summary");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_telnet_environment_has, 1);
  lua_setfield(state, -2, "telnet_environment_has");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_telnet_environment_get, 1);
  lua_setfield(state, -2, "telnet_environment_get");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_flow_start, 1);
  lua_setfield(state, -2, "flow_start");
}
