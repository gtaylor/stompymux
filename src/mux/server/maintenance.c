/* maintenance.c - Dependencies for periodic process maintenance. */

#include "mux/server/maintenance.h"

#include <assert.h>

void maintenance_context_initialize(
    MaintenanceContext *context, ServerControl *control,
    ConnectionRuntime *connections, const ServerConfiguration *configuration,
    GameDatabase *database, ServerLog *log, RuntimeClock *clock,
    DescriptorRegistry *descriptors, CommandQueue *commands,
    PlayerCache *players, CommandContext *command, BtechContext *btech,
    const LuaServices *lua_services, LuaOwner *lua) {
  assert(context != nullptr);
  context->control = control;
  context->connections = connections;
  context->configuration = configuration;
  context->database = database;
  context->log = log;
  context->clock = clock;
  context->descriptors = descriptors;
  context->commands = commands;
  context->players = players;
  context->command = command;
  context->btech = btech;
  context->lua_services = lua_services;
  context->lua = lua;
}
