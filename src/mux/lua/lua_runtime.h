/* lua_runtime.h - Lua runtime lifecycle and MUX callback declarations. */

#pragma once

#include "mux/database/db.h"

#include <stddef.h>
#include <time.h>

typedef struct LuaRuntime LuaRuntime;
typedef struct LuaServices LuaServices;
typedef struct LuaOwner LuaOwner;
typedef struct CommandContext CommandContext;
typedef struct CommandQueue CommandQueue;
typedef struct Descriptor Descriptor;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;

struct LuaServices {
  /* Every member is borrowed from MuxServer. */
  const ServerConfiguration *configuration;
  GameDatabase *database;
  DescriptorRegistry *descriptors;
  CommandQueue *commands;
  RuntimeClock *clock;
  CommandContext *background_command;
  ServerLog *log;
  int *record_players;
};

struct LuaOwner {
  LuaRuntime *runtime;
};

static inline void lua_services_initialize(
    LuaServices *services, const ServerConfiguration *configuration,
    GameDatabase *database, DescriptorRegistry *descriptors,
    CommandQueue *commands, RuntimeClock *clock,
    CommandContext *background_command, ServerLog *log, int *record_players) {
  *services = (LuaServices){
      .configuration = configuration,
      .database = database,
      .descriptors = descriptors,
      .commands = commands,
      .clock = clock,
      .background_command = background_command,
      .log = log,
      .record_players = record_players,
  };
}

int lua_initialize(LuaOwner *owner, const LuaServices *services, char *error,
                   size_t error_size);
void lua_shutdown(LuaOwner *owner);
int lua_reload(LuaOwner *owner, char *error, size_t error_size);
int lua_check(EvaluationContext *evaluation, LuaRuntime *source, DbRef player,
              char *error, size_t error_size);
int lua_validate_path(LuaRuntime *runtime, const char *path, char *error,
                      size_t error_size);
int lua_command_match(LuaRuntime *runtime, Descriptor *descriptor, DbRef thing,
                      DbRef player, DbRef cause, const char *command);
int lua_list_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                           DbRef first, DbRef player, DbRef cause,
                           const char *command);
int lua_global_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                             DbRef player, DbRef cause, const char *command);
int lua_event_dispatch(LuaRuntime *runtime, DbRef player, DbRef thing,
                       int attribute, char *args[], int nargs);
void lua_schedule_tick(LuaRuntime *runtime, time_t now);
