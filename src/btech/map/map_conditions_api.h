
/*
   p.map.conditions.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:42 CET 1999 from map.conditions.c */

#pragma once

#include "mux/server/platform.h"

/* map.conditions.c */
void alter_conditions(BattleMap *map);
void map_setconditions(DbRef player, BattleMap *map, char *buffer);
void UpdateConditions(Mech *mech, BattleMap *map);
void DestroyParts(Mech *attacker, Mech *wounded, int hitloc, int breach,
                  int IsDisable);
void reactor_explosion(Mech *wounded, Mech *attacker);

int BreachLoc(Mech *attacker, Mech *mech, int hitloc);
int PossiblyBreach(Mech *attacker, Mech *mech, int hitloc);
