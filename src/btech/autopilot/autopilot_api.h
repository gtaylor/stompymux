
/*
   p.autopilot.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:34 CET 1999 from autopilot.c */

#pragma once

#include "mux/server/platform.h"

typedef struct Mech Mech;
typedef struct MuxEvent MuxEvent;

/* autopilot.c */
void gradually_load(Mech *mech, int loc, int percent);
void autopilot_load_cargo(DbRef player, Mech *mech, int percent);
void figure_out_range_and_bearing(Mech *mech, int tx, int ty, float *range,
                                  int *bearing);
void auto_goto_event(MuxEvent *e);
void auto_follow_event(MuxEvent *e);
