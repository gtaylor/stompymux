/* maintenance.h - Dependencies for periodic process maintenance. */

#pragma once

typedef struct CommandContext CommandContext;
typedef struct CommandQueue CommandQueue;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct PlayerCache PlayerCache;
typedef struct RuntimeClock RuntimeClock;
typedef struct LuaRuntime LuaRuntime;
typedef struct MuxServer MuxServer;
typedef struct ServerConfiguration ServerConfiguration;

typedef struct MaintenanceContext MaintenanceContext;
struct MaintenanceContext {
  MuxServer *server;
  const ServerConfiguration *configuration;
  RuntimeClock *clock;
  DescriptorRegistry *descriptors;
  CommandQueue *commands;
  PlayerCache *players;
  CommandContext *command;
  LuaRuntime **lua;
};

void maintenance_context_initialize(
    MaintenanceContext *context, MuxServer *server,
    const ServerConfiguration *configuration,
    RuntimeClock *clock, DescriptorRegistry *descriptors,
    CommandQueue *commands, PlayerCache *players, CommandContext *command,
    LuaRuntime **lua);
