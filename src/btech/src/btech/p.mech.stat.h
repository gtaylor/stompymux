
/*
   p.mech.stat.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:57 CET 1999 from mech.stat.c */

#pragma once

typedef struct CommandInvocation CommandInvocation;

/* mech.stat.c */
void init_stat(BtechContext *context);
void do_show_stat(CommandInvocation *invocation);
