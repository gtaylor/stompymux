/* mux_server.h - Process composition root for owned MUX subsystems. */

#pragma once

#include <stdbool.h>
#include <time.h>

#include "btech/btech_context.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_runtime.h"
#include "mux/commands/macro.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/comsys_context.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/connection_runtime.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/configuration_context.h"
#include "mux/server/log.h"
#include "mux/server/maintenance.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_control.h"
#include "mux/world/world_context.h"

typedef struct CommandQueue CommandQueue;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct FileCache FileCache;
typedef struct HelpIndex HelpIndex;
typedef struct LoginThrottle LoginThrottle;
typedef struct LogCache LogCache;
typedef struct PlayerCache PlayerCache;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;

typedef struct MuxServer MuxServer;
struct MuxServer {
  char version[256];
  time_t start_time;
  time_t process_start_time;
  int record_players;
  ServerConfiguration *configuration;
  BtechContext btech;
  DescriptorRegistry *descriptors;
  CommandQueue *commands;
  ServerLifecycle *lifecycle;
  LoginThrottle *login_throttle;
  PlayerCache *players;
  FileCache *files;
  HelpIndex *help;
  ServerLog log;
  RuntimeClock clock;
  ChannelRegistry channels;
  ComsysContext comsys;
  GameDatabase database;
  MuxEventScheduler events;
  PersistenceContext persistence;
  MacroRegistry macros;
  CommandRegistry command_registry;
  WorldIndexes world_indexes;
  AccessControlStore access_control;
  WorldContext world;
  CommandContext background_command;
  ConfigurationContext configuration_context;
  ConnectionRuntime connection_runtime;
  ServerControl server_control;
  MaintenanceContext maintenance;
  CommandRuntime command_runtime;
  LuaServices lua_services;
  LuaOwner lua;
};

bool mux_server_create(MuxServer *server);
bool mux_server_load_content(MuxServer *server);
void mux_server_destroy(MuxServer *server);
