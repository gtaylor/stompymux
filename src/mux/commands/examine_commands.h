/** @file
 * Object examination and inventory command interface.
 */
#pragma once

#include "mux/commands/command_invocation.h"

typedef struct CommandInvocation CommandInvocation;

/** Handles the examine command. @param[in,out] invocation Command invocation.
 */

void do_examine(CommandInvocation *invocation);
/** Handles the inventory command. @param[in,out] invocation Command invocation.
 */

void do_inventory(CommandInvocation *invocation);
/** Handles the entrances command. @param[in,out] invocation Command invocation.
 */

void do_entrances(CommandInvocation *invocation);
