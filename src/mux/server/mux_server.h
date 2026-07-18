/* mux_server.h - Process composition root for owned MUX subsystems. */

#pragma once

#include <stdbool.h>
#include <time.h>

#include "btech/btech_context.h"
#include "mux/commands/command_context.h"
#include "mux/commands/macro.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/comsys_context.h"
#include "mux/database/db.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/mux_event.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/log.h"
#include "mux/server/maintenance.h"
#include "mux/server/runtime_clock.h"
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
typedef struct VattrStore VattrStore;

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
  VattrStore *vattrs;
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
  MaintenanceContext maintenance;
  LuaRuntime *lua;
};

bool mux_server_create(MuxServer *server);
bool mux_server_load_content(MuxServer *server);
void mux_server_destroy(MuxServer *server);
