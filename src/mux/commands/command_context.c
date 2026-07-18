/* command_context.c - Per-command evaluation-state lifecycle. */

#include "mux/commands/command_context.h"

#include <string.h>

#include "mux/database/db.h"
#include "mux/server/mux_server.h"
#include "mux/support/alloc.h"

bool command_context_initialize(CommandContext *context, MuxServer *server,
                                DbRef player, DbRef enactor,
                                Descriptor *descriptor, bool interactive) {
  memset(context, 0, sizeof(*context));
  context->server = server;
  context->world = &server->world;
  context->evaluation.world = &server->world;
  context->evaluation.command = context;
  context->match.evaluation = &context->evaluation;
  context->evaluation.server = server;
  context->player = player;
  context->enactor = enactor;
  context->descriptor = descriptor;
  context->interactive = interactive;
  context->debug_command = "< init >";
  context->evaluation.pipe_object = NOTHING;
  context->evaluation.trace_top = true;
  for (int index = 0; index < MAX_GLOBAL_REGS; index++) {
    context->evaluation.registers[index] = alloc_lbuf("command_context.reg");
    if (context->evaluation.registers[index] == nullptr) {
      command_context_destroy(context);
      return false;
    }
    context->evaluation.registers[index][0] = '\0';
  }
  return true;
}

void command_context_destroy(CommandContext *context) {
  if (context == nullptr)
    return;
  for (int index = 0; index < MAX_GLOBAL_REGS; index++) {
    if (context->evaluation.registers[index] != nullptr)
      free_lbuf(context->evaluation.registers[index]);
    context->evaluation.registers[index] = nullptr;
  }
  if (context->evaluation.pipe_output != nullptr)
    free_lbuf(context->evaluation.pipe_output);
  if (context->evaluation.pipe_next != nullptr &&
      context->evaluation.pipe_next != context->evaluation.pipe_output)
    free_lbuf(context->evaluation.pipe_next);
  memset(context, 0, sizeof(*context));
}

void command_context_reset_limits(CommandContext *context) {
  context->evaluation.function_nesting = 0;
  context->evaluation.function_invocations = 0;
  context->evaluation.notification_nesting = 0;
  context->evaluation.lock_nesting = 0;
}
