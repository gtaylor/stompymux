/* mux_server.c - Construction and teardown for the MUX composition root. */

#include "mux/server/mux_server.h"

#include <stdlib.h>
#include <string.h>

#include "mux/commands/command_queue.h"
#include "mux/database/vattr.h"
#include "mux/help/help_index.h"
#include "mux/network/connect_flow.h"
#include "mux/network/descriptor.h"
#include "mux/server/file_cache.h"
#include "mux/server/log_cache.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_config.h"
#include "mux/world/player_cache.h"

bool mux_server_create(MuxServer *server) {
  memset(server, 0, sizeof(*server));
  server->configuration = calloc(1, sizeof(*server->configuration));
  game_database_initialize(&server->database);
  runtime_clock_initialize(&server->clock);
  channel_registry_initialize(&server->channels);
  comsys_context_initialize(&server->comsys, server->configuration,
                            &server->clock, &server->channels);
  mux_event_scheduler_initialize(&server->events);
  macro_registry_initialize(&server->macros, &server->channels);
  command_registry_initialize(&server->command_registry);
  world_indexes_initialize(&server->world_indexes);
  access_control_store_initialize(&server->access_control);
  server->descriptors = descriptor_registry_create(server);
  world_context_initialize(&server->world, &server->database,
                           server->configuration, &server->world_indexes,
                           &server->access_control, server->descriptors);
  server->commands = command_queue_create(server);
  server->login_throttle = login_throttle_create();
  server->players =
      player_cache_create(server->configuration, &server->database);
  server->vattrs = vattr_store_create(&server->database);
  game_database_bind_services(&server->database, server->configuration,
                              &server->world_indexes, server->descriptors,
                              server->players, server->vattrs, &server->log);
  server_log_initialize(&server->log, &server->database,
                        server->configuration);
  bool command_context_ready = command_context_initialize(
      &server->background_command, server, NOTHING, NOTHING, nullptr, false);
  persistence_context_initialize(
      &server->persistence, server->configuration, &server->database,
      server->vattrs, &server->channels, &server->macros, &server->clock.now,
      &server->record_players, &server->world, &server->log);
  maintenance_context_initialize(
      &server->maintenance, server, server->configuration, &server->clock,
      server->descriptors, server->commands, server->players,
      &server->background_command, &server->lua);
  if (server->configuration != nullptr && server->descriptors != nullptr &&
      server->commands != nullptr && server->players != nullptr &&
      command_context_ready)
    server->lifecycle = server_lifecycle_create(&server->maintenance);
  if (server->lifecycle != nullptr)
    mux_event_scheduler_set_loop(&server->events,
                                 server_lifecycle_loop(server->lifecycle));
#ifdef ARBITRARY_LOGFILES
  if (server->lifecycle != nullptr)
    server->log.cache =
        log_cache_create(server_lifecycle_loop(server->lifecycle),
                         &server->log);
#endif
  btech_context_initialize(
      &server->btech, server->configuration, &server->clock,
      &server->background_command, &server->database, &server->events,
      server->lifecycle, &server->log, &server->persistence, &server->world_indexes,
      &server->access_control, server->process_start_time);
  btech_context_activate(&server->btech);
  if (server->configuration == nullptr ||
      server->descriptors == nullptr || server->commands == nullptr ||
      server->lifecycle == nullptr || server->login_throttle == nullptr ||
      server->players == nullptr || server->vattrs == nullptr ||
      !command_context_ready
#ifdef ARBITRARY_LOGFILES
      || server->log.cache == nullptr
#endif
  ) {
    mux_server_destroy(server);
    return false;
  }
  return true;
}

bool mux_server_load_content(MuxServer *server) {
  server->files = file_cache_create(&server->background_command.evaluation,
                                    server->configuration,
                                    server->descriptors);
  server->help = help_index_create(&server->background_command.evaluation,
                                   &server->log,
                                   server->configuration->help_dir, NOTHING);
  return server->files != nullptr && server->help != nullptr;
}

void mux_server_destroy(MuxServer *server) {
  if (server == nullptr)
    return;

  help_index_destroy(server->help);
  server->help = nullptr;
  command_queue_destroy(server->commands);
  server->commands = nullptr;
  log_cache_destroy(server->log.cache);
  server->log.cache = nullptr;
  server_lifecycle_shutdown(server->lifecycle);
  mux_event_scheduler_destroy(&server->events);
  macro_registry_destroy(&server->macros);
  channel_registry_destroy(&server->channels);
  descriptor_registry_destroy(server->descriptors);
  server->descriptors = nullptr;
  login_throttle_destroy(server->login_throttle);
  server->login_throttle = nullptr;
  player_cache_destroy(server->players);
  server->players = nullptr;
  vattr_store_destroy(server->vattrs);
  server->vattrs = nullptr;
  game_database_destroy(&server->database);
  command_context_destroy(&server->background_command);
  file_cache_destroy(server->files);
  server->files = nullptr;
  server_lifecycle_destroy(server->lifecycle);
  server->lifecycle = nullptr;
  btech_context_activate(nullptr);
  free(server->configuration);
  server->configuration = nullptr;
}
