/* Declares the BattleTech autopilot API. */

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
