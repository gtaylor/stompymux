/** @file
 * Inventory movement command interface.
 */
#pragma once

#include "mux/commands/command_invocation.h"

typedef struct CommandInvocation CommandInvocation;

constexpr int GET_QUIET = 1;  /* Suppress other text and success event. */
constexpr int DROP_QUIET = 1; /* Do not run drop actions if controlled. */

/** Handles the get command. @param[in] invocation Command invocation. */

void do_get(CommandInvocation *invocation);
/** Handles the drop command. @param[in,out] invocation Command invocation. */

void do_drop(CommandInvocation *invocation);
