/* Declares the BattleTech BattleTech API. */

#pragma once

#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;

/* btech.c */
void list_fhashstats(DbRef player);
void do_btech(CommandInvocation *invocation);
/** Applies a wizard administration command to the unit containing the player.
 * @param[in,out] invocation Command invocation. */
void do_mech_admin(CommandInvocation *invocation);
void do_show(CommandInvocation *invocation);
