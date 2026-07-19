/* configuration_context.h - Borrowed services used by configuration parsing. */

#pragma once

typedef struct CommandContext CommandContext;
typedef struct CommandRegistry CommandRegistry;
typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;
typedef struct WorldContext WorldContext;
typedef struct WorldIndexes WorldIndexes;

typedef struct ConfigurationContext ConfigurationContext;
struct ConfigurationContext {
  /* Every member is borrowed from MuxServer. */
  ServerConfiguration *configuration;
  GameDatabase *database;
  ServerLog *log;
  CommandContext *command;
  CommandRegistry *command_registry;
  WorldIndexes *world_indexes;
  WorldContext *world;
};

static inline void configuration_context_initialize(
    ConfigurationContext *context, ServerConfiguration *configuration,
    GameDatabase *database, ServerLog *log, CommandContext *command,
    CommandRegistry *command_registry, WorldIndexes *world_indexes,
    WorldContext *world) {
  *context = (ConfigurationContext){
      .configuration = configuration,
      .database = database,
      .log = log,
      .command = command,
      .command_registry = command_registry,
      .world_indexes = world_indexes,
      .world = world,
  };
}
