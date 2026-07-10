
/* p.mech.spot.h */

#pragma once

int IsArtyMech(MECH *mech);
void ClearFireAdjustments(MAP *map, dbref mech);
void mech_spot(dbref player, void *data, char *buffer);
int FireSpot(dbref player, MECH *mech, MAP *mech_map, int weaponnum,
             int weapontype, int sight, int section, int critical);
