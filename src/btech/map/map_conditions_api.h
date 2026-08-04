
/*
   p.map.conditions.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:42 CET 1999 from map.conditions.c */

#pragma once

#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

/* map.conditions.c */
void alter_conditions(BattleMap *map);
void map_setconditions(DbRef player, BattleMap *map, char *buffer);
void map_conditions_apply(Mech *mech, BattleMap *map);
