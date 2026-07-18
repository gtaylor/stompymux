/* btech_context.c - Runtime dependency bridge for legacy BTech callbacks. */

#include "btech/btech_context.h"

#include <assert.h>

static BtechContext *active_context = nullptr;

void btech_context_initialize(
    BtechContext *context, ServerConfiguration *configuration,
    RuntimeClock *clock, CommandContext *command_context, GameDatabase *database,
    MuxEventScheduler *events, ServerLifecycle *lifecycle, ServerLog *log,
    PersistenceContext *persistence,
    WorldIndexes *world_indexes, AccessControlStore *access_control,
    time_t process_start_time) {
  *context = (BtechContext){
      .configuration = configuration,
      .clock = clock,
      .command_context = command_context,
      .database = database,
      .events = events,
      .lifecycle = lifecycle,
      .log = log,
      .persistence = persistence,
      .world_indexes = world_indexes,
      .access_control = access_control,
      .process_start_time = process_start_time,
  };
}

void btech_context_activate(BtechContext *context) { active_context = context; }

BtechContext *btech_context_active(void) {
  assert(active_context != nullptr);
  return active_context;
}

CommandContext *btech_context_set_command(BtechContext *context,
                                          CommandContext *command_context) {
  CommandContext *previous = context->command_context;
  context->command_context = command_context;
  return previous;
}
