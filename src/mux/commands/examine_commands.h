/* Object examination and inventory command interface. */

#pragma once

#include "mux/commands/command_invocation.h"

typedef struct CommandInvocation CommandInvocation;

void do_examine(CommandInvocation *invocation);
void do_inventory(CommandInvocation *invocation);
void do_entrances(CommandInvocation *invocation);
