#pragma once

#include <stddef.h>

typedef struct lua_State lua_State;
typedef int (*lua_CFunction)(lua_State *state);

void lua_pushcclosure(lua_State *state, lua_CFunction function, int upvalues);
void lua_pushstring(lua_State *state, const char *value);
void lua_setfield(lua_State *state, int index, const char *name);
void lua_setglobal(lua_State *state, const char *name);

#define lua_pushcfunction(state, function)                                     \
  lua_pushcclosure((state), (function), 0)

typedef struct BtechLuaEntry {
  const char *name;
  const char *qualified_name;
  int (*handler)(void *call);
} BtechLuaEntry;

void lua_btech_install_bindings(lua_State *state, void *package,
                                const char *name, const BtechLuaEntry *entries,
                                size_t entry_count);

typedef struct ChannelFlagDefinition {
  int value;
  const char *name;
} ChannelFlagDefinition;

typedef struct LuaMuxChannelMethod {
  const char *name;
  lua_CFunction function;
} LuaMuxChannelMethod;

typedef struct POWERENT {
  const char *powername;
  int id;
  int permissions;
} POWERENT;

typedef struct FixtureCatalogEntry {
  int value;
  const char *key;
  const char *name;
} FixtureCatalogEntry;
