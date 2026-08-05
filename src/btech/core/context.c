/* context.c - Runtime dependency bridge for legacy BTech callbacks. */

#include "btech/context.h"

#include <assert.h>
#include <stdlib.h>

#include "mux/commands/command_context.h"
#include "mux/network/mux_event.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_config.h"

BtechContext *btech_context_create(const BtechDependencies *dependencies) {
  if (dependencies == nullptr)
    return nullptr;
  BtechContext *context = calloc(1, sizeof(*context));
  if (context == nullptr)
    return nullptr;
  *context = (BtechContext){
      .dependencies = *dependencies,
      .configuration = dependencies->configuration,
      .clock = dependencies->clock,
      .background_command = dependencies->background_command,
      .database = dependencies->database,
      .events = dependencies->events,
      .lifecycle = dependencies->lifecycle,
      .log = dependencies->log,
      .persistence = dependencies->persistence,
      .world_indexes = dependencies->world_indexes,
      .access_control = dependencies->access_control,
      .process_start_time = dependencies->process_start_time,
      .cached_target_character = -1,
  };
  return context;
}

void btech_context_destroy(BtechContext *context) {
  if (context == nullptr)
    return;
  btech_context_release_owned_state(context);
  free(context);
}

void btech_context_set_lifecycle(BtechContext *context,
                                 ServerLifecycle *lifecycle) {
  if (context == nullptr)
    return;
  context->dependencies.lifecycle = lifecycle;
  context->lifecycle = lifecycle;
}

void btech_context_set_process_start_time(BtechContext *context,
                                          time_t process_start_time) {
  assert(context != nullptr);
  context->dependencies.process_start_time = process_start_time;
  context->process_start_time = process_start_time;
}

CommandContext *btech_context_command(BtechContext *context) {
  assert(context != nullptr);
  return context->command_scope != nullptr ? context->command_scope->command
                                           : context->background_command;
}

EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return &btech_context_command(context)->evaluation;
}

GameDatabase *btech_context_database(BtechContext *context) {
  assert(context != nullptr);
  return context->database;
}

bool btech_context_combat_arcs_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->combat_overrides.arcs;
}

void btech_context_combat_arcs_override_set(BtechContext *context, int arcs) {
  assert(context != nullptr);
  context->combat_overrides.arcs = arcs;
}

void btech_context_combat_pilot_override_set(BtechContext *context,
                                             BtechObjectId pilot) {
  assert(context != nullptr);
  context->combat_overrides.pilot = pilot;
}

bool btech_context_seismic_detects_stopped_units(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_seismic_see_stopped;
}

int btech_context_movement_slowdown_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_slowdown;
}

int btech_context_event_tick(const BtechContext *context) {
  assert(context != nullptr);
  return context->events->tick;
}

time_t btech_context_now(const BtechContext *context) {
  assert(context != nullptr);
  return context->clock->now;
}

void btech_command_scope_enter(BtechCommandScope *scope, BtechContext *context,
                               CommandContext *command) {
  assert(scope != nullptr);
  assert(context != nullptr);
  assert(command != nullptr);
  *scope = (BtechCommandScope){
      .context = context,
      .command = command,
      .previous = context->command_scope,
      .active = true,
  };
  context->command_scope = scope;
}

void btech_command_scope_leave(BtechCommandScope *scope) {
  assert(scope != nullptr);
  assert(scope->active);
  assert(scope->context->command_scope == scope);
  scope->context->command_scope = scope->previous;
  *scope = (BtechCommandScope){0};
}
