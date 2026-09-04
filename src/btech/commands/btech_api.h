/* Declares the BattleTech BattleTech API. */

#pragma once

#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;

/* btech.c */
void list_fhashstats(DbRef player);
void do_btech(CommandInvocation *invocation);
void do_show(CommandInvocation *invocation);
