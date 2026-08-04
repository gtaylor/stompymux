
/* p.mech.spot.h */

#pragma once

#include "mux/server/platform.h"

int IsArtyMech(Mech *mech);
void ClearFireAdjustments(BattleMap *map, DbRef mech);
void mech_spot(DbRef player, void *data, char *buffer);
int FireSpot(DbRef player, Mech *mech, BattleMap *mech_map, int weaponnum,
             int weapontype, int sight, int section, int critical);
