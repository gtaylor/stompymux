/* mux_package.c - Built-in Lua mux package bindings. */

#include "mux/server/platform.h"

#include "mux/lua/mux_package.h"

#include <lauxlib.h>

#include "mux/commands/command_queue.h"
#include "mux/database/attrs.h"
#include "mux/database/flags.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"

static LuaMuxPackage *lua_mux_package_get(lua_State *state) {
  return lua_touserdata(state, lua_upvalueindex(1));
}

static int lua_mux_package_is_checking(LuaMuxPackage *package) {
  return package->is_checking && package->is_checking(package->context);
}

static int lua_mux_attr_get(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object = (DbRef)luaL_checkinteger(state, 1);
  const char *name = luaL_checkstring(state, 2);
  Attribute *attribute;
  char *value;
  DbRef owner;
  long flags;

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.attr_get is unavailable during @lua/check");
  if (!is_good_obj(package->services->database, object))
    return luaL_error(state, "invalid object");
  attribute = attribute_by_name(package->services->database, name);
  if (!attribute) {
    lua_pushnil(state);
    return 1;
  }
  value = attribute_get(package->services->database, object, attribute->number,
                        &owner, &flags);
  if (!*value)
    lua_pushnil(state);
  else
    lua_pushstring(state, value);
  free_lbuf(value);
  return 1;
}

static int lua_mux_attr_set(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object = (DbRef)luaL_checkinteger(state, 1);
  const char *name = luaL_checkstring(state, 2);
  const char *value = luaL_checkstring(state, 3);
  char attribute_name[SBUF_SIZE];
  int attribute;

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.attr_set is unavailable during @lua/check");
  if (!is_good_obj(package->services->database, object))
    return luaL_error(state, "invalid object");
  snprintf(attribute_name, sizeof(attribute_name), "%s", name);
  attribute = mkattr(package->services->database, attribute_name);
  if (attribute < 0)
    return luaL_error(state, "invalid attribute");
  if (attribute == A_LUAPARENT)
    return luaL_error(state, "use @lua/parent to change Luaparent");
  /* attribute_add_raw()'s buffer parameter isn't const-correct; value is
     only read (copied) here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  attribute_add_raw(package->services->database, object, attribute,
                    (char *)value);
#pragma clang diagnostic pop
  return 0;
}

static int lua_mux_notify(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object = (DbRef)luaL_checkinteger(state, 1);
  const char *message = luaL_checkstring(state, 2);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.notify is unavailable during @lua/check");
  if (!is_good_obj(package->services->database, object))
    return luaL_error(state, "invalid object");
  notify(&package->services->background_command->evaluation, object, message);
  return 0;
}

static int lua_mux_command(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *command = luaL_checkstring(state, 1);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.command is unavailable during @lua/check");
  /* wait_que()'s command parameter isn't const-correct; command is only
     read here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  wait_que(package->services->commands, 1, 1, 0, NOTHING, 0, (char *)command,
           (char **)nullptr, 0, nullptr);
#pragma clang diagnostic pop
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
    if (package->services->configuration->show_unfindable_who &&
        is_hidden(package->services->database, descriptor->player))
      continue;
    lua_newtable(state);
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
  Descriptor *descriptor;
  DescriptorIterator iterator =
      descriptor_iterator_connected(package->services->descriptors);
  int hidden = 0;

  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    if (package->services->configuration->show_unfindable_who &&
        is_hidden(package->services->database, descriptor->player))
      hidden++;
  }
  lua_newtable(state);
  lua_pushinteger(state, hidden);
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

static int lua_mux_flow_start(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  int descriptor_id = (int)luaL_checkinteger(state, 1);
  const char *module = luaL_checkstring(state, 2);
  const char *first_step = luaL_checkstring(state, 3);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.flow_start is unavailable during @lua/check");
  if (!package->flow_start)
    return luaL_error(state, "mux.flow_start is unavailable");
  return package->flow_start(package->context, state, descriptor_id, module,
                             first_step);
}

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package) {
  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_attr_get, 1);
  lua_setfield(state, -2, "attr_get");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_attr_set, 1);
  lua_setfield(state, -2, "attr_set");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_notify, 1);
  lua_setfield(state, -2, "notify");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_command, 1);
  lua_setfield(state, -2, "command");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_connected_players, 1);
  lua_setfield(state, -2, "connected_players");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_who_summary, 1);
  lua_setfield(state, -2, "who_summary");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_flow_start, 1);
  lua_setfield(state, -2, "flow_start");
  lua_setglobal(state, "mux");
}
