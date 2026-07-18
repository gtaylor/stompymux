/* help_command.h - `help` and `@helpreload` command handlers. */

#pragma once

#include "mux/commands/command_invocation.h"

void do_help(CommandInvocation *invocation);
void do_helpreload(CommandInvocation *invocation);
