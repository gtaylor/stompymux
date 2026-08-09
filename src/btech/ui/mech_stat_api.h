/* Declares the BattleTech unit stat API. */

#pragma once

typedef struct BtechContext BtechContext;

typedef struct CommandInvocation CommandInvocation;

/* mech.stat.c */
void init_stat(BtechContext *context);
void do_show_stat(CommandInvocation *invocation);
