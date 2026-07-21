/* command_context.c - Per-command evaluation-state lifecycle. */

#include "mux/commands/command_context.h"

#include <string.h>

#include "mux/commands/command_runtime.h"
#include "mux/database/db.h"

bool command_context_initialize(CommandContext *context,
                                CommandRuntime *runtime, BtechContext *btech,
                                ServerLog *log, DbRef player, DbRef enactor,
                                Descriptor *descriptor, bool interactive) {
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
  context->player = player;
  context->enactor = enactor;
  context->descriptor = descriptor;
  context->interactive = interactive;
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
