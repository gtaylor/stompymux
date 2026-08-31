/* lua_lock_catalog.c - Canonical Lua object-lock catalog. */

#include "mux/lua/lua_lock_catalog.h"

#include <string.h>

#include "mux/lua/lua_runtime.h"
#include "mux/support/checked_storage.h"

static const LuaLockDefinition LUA_LOCK_DEFINITIONS[] = {
    {LUA_LOCK_MATCH, "match", "MATCH"},
    {LUA_LOCK_TRAVERSE, "traverse", "TRAVERSE"},
    {LUA_LOCK_TAKE, "take", "TAKE"},
    {LUA_LOCK_USE, "use", "USE"},
    {LUA_LOCK_DROP, "drop", "DROP"},
    {LUA_LOCK_GIVE, "give", "GIVE"},
    {LUA_LOCK_RECEIVE, "receive", "RECEIVE"},
    {LUA_LOCK_ENTER, "enter", "ENTER"},
    {LUA_LOCK_LEAVE, "leave", "LEAVE"},
    {LUA_LOCK_TELEPORT, "teleport", "TELEPORT"},
    {LUA_LOCK_TELEPORT_OUT, "teleport_out", "TELEPORT_OUT"},
    {LUA_LOCK_LINK, "link", "LINK"},
    {LUA_LOCK_SET_HOME, "set_home", "SET_HOME"},
    {LUA_LOCK_SPEAK, "speak", "SPEAK"},
    {LUA_LOCK_CHANNEL_JOIN, "channel_join", "CHANNEL_JOIN"},
    {LUA_LOCK_CHANNEL_TRANSMIT, "channel_transmit", "CHANNEL_TRANSMIT"},
    {LUA_LOCK_CHANNEL_RECEIVE, "channel_receive", "CHANNEL_RECEIVE"},
    {LUA_LOCK_IDENTIFY_BUILDING, "identify_building", "IDENTIFY_BUILDING"},
};

static_assert(sizeof(LUA_LOCK_DEFINITIONS) / sizeof(*LUA_LOCK_DEFINITIONS) ==
              LUA_LOCK_COUNT);

size_t lua_lock_definition_count(void) {
  return sizeof(LUA_LOCK_DEFINITIONS) / sizeof(*LUA_LOCK_DEFINITIONS);
}

const LuaLockDefinition *lua_lock_definition_at(size_t index) {
  return checked_storage_at_const(LUA_LOCK_DEFINITIONS,
                                  lua_lock_definition_count(),
                                  sizeof(*LUA_LOCK_DEFINITIONS), index);
}

const LuaLockDefinition *lua_lock_definition_find_key(const char *key) {
  if (!key)
    return nullptr;
  for (size_t index = 0; index < lua_lock_definition_count(); index++) {
    const LuaLockDefinition *definition = lua_lock_definition_at(index);

    if (!strcmp(definition->key, key))
      return definition;
  }
  return nullptr;
}

const LuaLockDefinition *
lua_lock_definition_find_constant(const char *constant) {
  if (!constant)
    return nullptr;
  for (size_t index = 0; index < lua_lock_definition_count(); index++) {
    const LuaLockDefinition *definition = lua_lock_definition_at(index);

    if (!strcmp(definition->constant, constant))
      return definition;
  }
  return nullptr;
}

const char *lua_lock_name(LuaLockType lock) {
  if ((unsigned int)lock >= LUA_LOCK_COUNT)
    return nullptr;
  const LuaLockDefinition *definition = lua_lock_definition_at((size_t)lock);

  return definition->type == lock ? definition->key : nullptr;
}
