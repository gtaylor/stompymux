/* Inventory movement command interface. */

#pragma once

typedef struct CommandInvocation CommandInvocation;

void do_get(CommandInvocation *invocation);
void do_drop(CommandInvocation *invocation);
