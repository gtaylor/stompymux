
/*
   p.mech.los.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:13 CET 1999 from mech.los.c */

#pragma once

#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

/* mech.los.c */
float mech_los_actual_elevation(BattleMap *map, int x, int y, Mech *mech);
int mech_los_calculate_flags(Mech *mech, Mech *target, BattleMap *map, int x,
                             int y, int previous_flags, float hex_range);
int mech_los_terrain_modifier(Mech *mech, Mech *target, BattleMap *map,
                              float hex_range, int ammunition_mode);
int mech_los_check_unblocked(Mech *mech, Mech *target, int x, int y,
                             float hex_range);
int mech_los_check(Mech *mech, Mech *target, int x, int y, float hex_range);
void mech_losemit(DbRef player, Mech *mech, char *buffer);
