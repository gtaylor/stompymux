
/*
   p.mech.los.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:13 CET 1999 from mech.los.c */

#pragma once

#include "mux/server/platform.h"

/* mech.los.c */
float ActualElevation(BattleMap *map, int x, int y, Mech *mech);
int CalculateLOSFlag(Mech *mech, Mech *target, BattleMap *map, int x, int y,
                     int ff, float hexRange);
int AddTerrainMod(Mech *mech, Mech *target, BattleMap *map, float hexRange,
                  int wAmmoMode);
int InLineOfSight_NB(Mech *mech, Mech *target, int x, int y, float hexRange);
int InLineOfSight(Mech *mech, Mech *target, int x, int y, float hexRange);
void mech_losemit(DbRef player, Mech *mech, char *buffer);
