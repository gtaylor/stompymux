/* maintenance.h - Dependencies for periodic process maintenance. */

#pragma once

typedef struct CommandContext CommandContext;
typedef struct BtechContext BtechContext;
typedef struct CommandQueue CommandQueue;
typedef struct ConnectionRuntime ConnectionRuntime;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct GameDatabase GameDatabase;
typedef struct PlayerCache PlayerCache;
typedef struct RuntimeClock RuntimeClock;
typedef struct LuaOwner LuaOwner;
typedef struct LuaServices LuaServices;
typedef struct ServerControl ServerControl;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;

typedef struct MaintenanceContext MaintenanceContext;
struct MaintenanceContext {
  /* Every member is borrowed from MuxServer. */
  ServerControl *control;
  ConnectionRuntime *connections;
  const ServerConfiguration *configuration;
  GameDatabase *database;
  ServerLog *log;
  RuntimeClock *clock;
  DescriptorRegistry *descriptors;
  CommandQueue *commands;
  PlayerCache *players;
  CommandContext *command;
  BtechContext *btech;
  const LuaServices *lua_services;
  /* Stable owner: reload replaces the runtime stored in this wrapper. */
  LuaOwner *lua;
};

void maintenance_context_initialize(
    MaintenanceContext *context, ServerControl *control,
    ConnectionRuntime *connections, const ServerConfiguration *configuration,
    GameDatabase *database, ServerLog *log, RuntimeClock *clock,
    DescriptorRegistry *descriptors, CommandQueue *commands,
    PlayerCache *players, CommandContext *command, BtechContext *btech,
    const LuaServices *lua_services, LuaOwner *lua);
