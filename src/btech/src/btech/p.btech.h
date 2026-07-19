
/*
   p.btech.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:36 CET 1999 from btech.c */

#pragma once

#include "mux/server/platform.h"

typedef struct CommandInvocation CommandInvocation;

/* btech.c */
void list_fhashstats(DbRef player);
void do_show(CommandInvocation *invocation);
