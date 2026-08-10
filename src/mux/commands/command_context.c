/* command_context.c - Per-command evaluation-state lifecycle. */

#include "mux/commands/command_context.h"

#include <string.h>

#include "mux/commands/command_queue.h"
#include "mux/objects/db.h"

bool command_context_initialize(
    const CommandContextInitialization *initialization) {
  CommandContext *context = initialization->context;
  CommandRuntime *runtime = initialization->runtime;
  BtechContext *btech = initialization->btech;
  ServerLog *log = initialization->log;
  memset(context, 0, sizeof(*context));
  context->runtime = runtime;
  context->btech = btech;
  context->log = log;
  context->world = runtime->world;
  context->evaluation.log = log;
  context->evaluation.world = runtime->world;
  context->evaluation.command = context;
  context->match.evaluation = &context->evaluation;
  context->evaluation.runtime = runtime;
  context->evaluation.btech = btech;
  context->player = initialization->player;
  context->enactor = initialization->enactor;
  context->descriptor = initialization->descriptor;
  context->interactive = initialization->interactive;
  context->debug_command = "< init >";
  return true;
}

void command_context_destroy(CommandContext *context) {
  if (context == nullptr)
    return;
  memset(context, 0, sizeof(*context));
}

void command_context_reset_limits(CommandContext *context) {
  context->evaluation.notification_nesting = 0;
}
