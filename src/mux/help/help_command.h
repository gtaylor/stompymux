/** @file
 * `help` and `@help` command handlers.
 */
#pragma once

#include "mux/commands/command_invocation.h"

typedef enum {
  HELP_COMMAND_RELOAD = 1 << 0,
} HelpCommandKey;

/** Handles the help command. @param[in,out] invocation Command invocation. */

void do_help(CommandInvocation *invocation);
/** Handles the help admin command. @param[in,out] invocation Command
 * invocation. */

void do_help_admin(CommandInvocation *invocation);
