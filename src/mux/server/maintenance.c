/* maintenance.c - Dependencies for periodic process maintenance. */

#include "mux/server/maintenance.h"

#include <assert.h>

void maintenance_context_initialize(
    MaintenanceContext *context, MuxServer *server,
    const ServerConfiguration *configuration,
    RuntimeClock *clock, DescriptorRegistry *descriptors,
    CommandQueue *commands, PlayerCache *players, CommandContext *command,
    LuaRuntime **lua) {
  assert(context != nullptr);
  context->server = server;
  context->configuration = configuration;
  context->clock = clock;
  context->descriptors = descriptors;
  context->commands = commands;
  context->players = players;
  context->command = command;
  context->lua = lua;
}
