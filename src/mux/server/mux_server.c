/* mux_server.c - Construction and teardown for the MUX composition root. */

#include "mux/server/mux_server.h"

#include <string.h>

#include "mux/commands/command_queue.h"
#include "mux/database/vattr.h"
#include "mux/help/help_index.h"
#include "mux/network/connect_flow.h"
#include "mux/network/descriptor.h"
#include "mux/server/configuration.h"
#include "mux/server/file_cache.h"
#include "mux/server/log_cache.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/world/player_cache.h"

bool mux_server_create(MuxServer *server) {
  memset(server, 0, sizeof(*server));
  server->configuration = server_configuration_create();
  if (server->configuration == nullptr)
    return false;

  /* Initialize embedded owners before publishing borrowed views of them. */
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
  server_log_initialize(&server->log, &server->database, server->configuration);

  /* Create opaque owners. Their addresses remain stable while service views
     are wired below. */
  server->descriptors = descriptor_registry_create(
      &server->command_runtime, &server->btech, &server->log);
  world_context_initialize(&server->world, &server->database,
                           server->configuration, &server->world_indexes,
                           &server->access_control, server->descriptors);
  server->login_throttle = login_throttle_create();
  server->players =
      player_cache_create(server->configuration, &server->database);
  server->vattrs = vattr_store_create(&server->database);
  const CommandQueueDependencies queue_dependencies = {
      .command_runtime = &server->command_runtime,
      .btech = &server->btech,
      .log = &server->log,
      .world = &server->world,
      .clock = &server->clock,
      .players = server->players,
      .background_command = &server->background_command,
  };
  server->commands = command_queue_create(&queue_dependencies);
  if (server->descriptors == nullptr || server->commands == nullptr ||
      server->login_throttle == nullptr || server->players == nullptr ||
      server->vattrs == nullptr)
    goto fail;
  game_database_bind_services(&server->database, server->configuration,
                              &server->world_indexes, server->descriptors,
                              server->players, server->vattrs, &server->log);

  /* Wire non-owning runtime views after every dependency they borrow has a
     stable address. */
  configuration_context_initialize(
      &server->configuration_context, server->configuration, &server->database,
      &server->log, &server->background_command, &server->command_registry,
      &server->world_indexes, &server->world);
  server_control_initialize(
      &server->server_control, server->configuration, &server->database,
      &server->log, server->descriptors, server->players, &server->persistence,
      &server->background_command, &server->btech);
  connection_runtime_initialize(&server->connection_runtime,
                                server->configuration, &server->clock,
                                server->descriptors, &server->log,
                                &server->access_control, &server->files);
  server->command_runtime = (CommandRuntime){
      .configuration_context = &server->configuration_context,
      .server_control = &server->server_control,
      .channels = &server->channels,
      .descriptors = server->descriptors,
      .clock = &server->clock,
      .commands = server->commands,
      .command_registry = &server->command_registry,
      .macros = &server->macros,
      .players = server->players,
      .login_throttle = server->login_throttle,
      .world_indexes = &server->world_indexes,
      .world = &server->world,
      .vattrs = server->vattrs,
      .background_command = &server->background_command,
      .lua_owner = &server->lua,
      .version = server->version,
      .start_time = &server->start_time,
      .record_players = &server->record_players,
  };
  bool command_context_ready = command_context_initialize(
      &server->background_command, &server->command_runtime, &server->btech,
      &server->log, NOTHING, NOTHING, nullptr, false);
  if (!command_context_ready)
    goto fail;
  persistence_context_initialize(
      &server->persistence, server->configuration, &server->database,
      server->vattrs, &server->channels, &server->macros, &server->clock.now,
      &server->record_players, &server->world, &server->log);
  lua_services_initialize(
      &server->lua_services, server->configuration, &server->database,
      server->descriptors, server->commands, &server->clock,
      &server->background_command, &server->log, &server->record_players);
  maintenance_context_initialize(
      &server->maintenance, &server->server_control,
      &server->connection_runtime, server->configuration, &server->database,
      &server->log, &server->clock, server->descriptors, server->commands,
      server->players, &server->background_command, &server->btech,
      &server->lua_services, &server->lua);
  server->lifecycle = server_lifecycle_create(&server->maintenance);
  if (server->lifecycle == nullptr)
    goto fail;
  if (server->lifecycle != nullptr)
    mux_event_scheduler_set_loop(&server->events,
                                 server_lifecycle_loop(server->lifecycle));
  server->server_control.lifecycle = server->lifecycle;
  command_queue_set_lifecycle(server->commands, server->lifecycle);
#ifdef ARBITRARY_LOGFILES
  server->log.cache =
      log_cache_create(server_lifecycle_loop(server->lifecycle), &server->log);
  if (server->log.cache == nullptr)
    goto fail;
#endif
  btech_context_initialize(&server->btech, server->configuration,
                           &server->clock, &server->background_command,
                           &server->database, &server->events,
                           server->lifecycle, &server->log,
                           &server->persistence, &server->world_indexes,
                           &server->access_control, server->process_start_time);
  server->command_runtime.lifecycle = server->lifecycle;
  return true;

fail:
  mux_server_destroy(server);
  return false;
}

bool mux_server_load_content(MuxServer *server) {
  server->files = file_cache_create(&server->background_command.evaluation,
                                    server->configuration, server->descriptors);
  server->help =
      help_index_create(&server->background_command.evaluation, &server->log,
                        server->configuration->help_dir, NOTHING);
  server->command_runtime.files = server->files;
  server->command_runtime.help = server->help;
  return server->files != nullptr && server->help != nullptr;
}

void mux_server_destroy(MuxServer *server) {
  if (server == nullptr)
    return;

  /* Every teardown below accepts its zero/null state, so this same path also
     unwinds any partially completed mux_server_create(). */
  /* Release the log timer while its libuv loop is still live. */
  log_cache_destroy(server->log.cache);
  server->log.cache = nullptr;
  /* Stop libuv producers and drain pending handles before their borrowed
     registries and queues are released below. */
  server_lifecycle_shutdown(server->lifecycle);
  server_lifecycle_destroy(server->lifecycle);
  server->lifecycle = nullptr;
  server->server_control.lifecycle = nullptr;
  server->command_runtime.lifecycle = nullptr;
  server->btech.lifecycle = nullptr;
  help_index_destroy(server->help);
  server->help = nullptr;
  file_cache_destroy(server->files);
  server->files = nullptr;
  command_queue_destroy(server->commands);
  server->commands = nullptr;
  btech_context_destroy(&server->btech);
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
  /* Per-command buffers borrow the world and database, so release them before
     destroying either service. */
  command_context_destroy(&server->background_command);
  game_database_destroy(&server->database);
  world_indexes_destroy(&server->world_indexes);
  command_registry_destroy(&server->command_registry);
  access_control_store_destroy(&server->access_control);
  server_configuration_destroy(server->configuration);
  server->configuration = nullptr;
}
