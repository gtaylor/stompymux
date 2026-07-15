
/* p.mech.spot.h */

#pragma once

int IsArtyMech(MECH *mech);
void ClearFireAdjustments(MAP *map, DbRef mech);
void mech_spot(DbRef player, void *data, char *buffer);
int FireSpot(DbRef player, MECH *mech, MAP *mech_map, int weaponnum,
             int weapontype, int sight, int section, int critical);
