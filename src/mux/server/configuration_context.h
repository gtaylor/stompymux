/** @file
 * Borrowed services used by configuration parsing.
 */
#pragma once

#include <stdbool.h>

typedef struct CommandContext CommandContext;
typedef struct CommandRegistry CommandRegistry;
typedef struct ConfigurationRegistry ConfigurationRegistry;
typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;
typedef struct WorldContext WorldContext;
typedef struct WorldIndexes WorldIndexes;

typedef struct ConfigurationContext ConfigurationContext;
struct ConfigurationContext {
  /* Service members are borrowed from MuxServer. */
  bool fatal_error;
  ServerConfiguration *configuration;
  GameDatabase *database;
  ServerLog *log;
  CommandContext *command;
  CommandRegistry *command_registry;
  ConfigurationRegistry *configuration_registry;
  WorldIndexes *world_indexes;
  WorldContext *world;
};

/** Initializes configuration context. @param[out] context Operation context.
 * @param[in] configuration Server configuration. @param[in] database Game
 * database. @param[in] log Server log. @param[in] command Command text or
 * descriptor. @param[in] command_registry Command registry. @param[in]
 * configuration_registry Configuration registry. @param[in] world_indexes World
 * indexes. @param[in] world World. */

static inline void configuration_context_initialize(
    ConfigurationContext *context, ServerConfiguration *configuration,
    GameDatabase *database, ServerLog *log, CommandContext *command,
    CommandRegistry *command_registry,
    ConfigurationRegistry *configuration_registry, WorldIndexes *world_indexes,
    WorldContext *world) {
  *context = (ConfigurationContext){
      .configuration = configuration,
      .database = database,
      .log = log,
      .command = command,
      .command_registry = command_registry,
      .configuration_registry = configuration_registry,
      .world_indexes = world_indexes,
      .world = world,
  };
}
