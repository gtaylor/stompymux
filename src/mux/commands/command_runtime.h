/* command_runtime.h - Borrowed services available during command execution. */

#pragma once

#include <time.h>

#include "mux/world/world_context.h"

typedef struct ChannelRegistry ChannelRegistry;
typedef struct CommandContext CommandContext;
typedef struct CommandQueue CommandQueue;
typedef struct CommandRegistry CommandRegistry;
typedef struct ConfigurationContext ConfigurationContext;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct FileCache FileCache;
typedef struct HelpIndex HelpIndex;
typedef struct LuaOwner LuaOwner;
typedef struct LoginThrottle LoginThrottle;
typedef struct MacroRegistry MacroRegistry;
typedef struct PlayerCache PlayerCache;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerControl ServerControl;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct VattrStore VattrStore;
typedef struct WorldIndexes WorldIndexes;

typedef struct CommandRuntime CommandRuntime;
struct CommandRuntime {
  /* Every member is borrowed from MuxServer. */
  ConfigurationContext *configuration_context;
  ServerControl *server_control;
  ChannelRegistry *channels;
  DescriptorRegistry *descriptors;
  RuntimeClock *clock;
  CommandQueue *commands;
  CommandRegistry *command_registry;
  MacroRegistry *macros;
  PlayerCache *players;
  LoginThrottle *login_throttle;
  WorldIndexes *world_indexes;
  WorldContext *world;
  FileCache *files;
  HelpIndex *help;
  VattrStore *vattrs;
  ServerLifecycle *lifecycle;
  CommandContext *background_command;
  /* Borrowed stable owner for a reloadable runtime. */
  LuaOwner *lua_owner;
  char *version;
  time_t *start_time;
  int *record_players;
};
