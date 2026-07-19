/* btech_context.c - Runtime dependency bridge for legacy BTech callbacks. */

#include "btech/btech_context.h"

#include "mux/commands/command_context.h"

#include <assert.h>

void btech_context_initialize(
    BtechContext *context, ServerConfiguration *configuration,
    RuntimeClock *clock, CommandContext *command_context,
    GameDatabase *database, MuxEventScheduler *events,
    ServerLifecycle *lifecycle, ServerLog *log, PersistenceContext *persistence,
    WorldIndexes *world_indexes, AccessControlStore *access_control,
    time_t process_start_time) {
  *context = (BtechContext){
      .configuration = configuration,
      .clock = clock,
      .background_command = command_context,
      .database = database,
      .events = events,
      .lifecycle = lifecycle,
      .log = log,
      .persistence = persistence,
      .world_indexes = world_indexes,
      .access_control = access_control,
      .process_start_time = process_start_time,
      .cached_target_character = -1,
  };
}

CommandContext *btech_context_command(BtechContext *context) {
  assert(context != nullptr);
  return context->command_scope != nullptr ? context->command_scope->command
                                           : context->background_command;
}

EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return &btech_context_command(context)->evaluation;
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
