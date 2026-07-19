/* help_command.h - `help` and `@help` command handlers. */

#pragma once

#include "mux/commands/command_invocation.h"

typedef enum {
  HELP_COMMAND_RELOAD = 1 << 0,
} HelpCommandKey;

void do_help(CommandInvocation *invocation);
void do_help_admin(CommandInvocation *invocation);
