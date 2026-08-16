
/* p.mech.spot.h */

#pragma once

#include <stdbool.h>

#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

bool mech_spot_has_artillery(Mech *mech);
void mech_spot_clear_fire_adjustments(BattleMap *map, DbRef mech);
void mech_spot(DbRef player, Mech *mech, char *buffer);
int mech_spot_fire(DbRef player, Mech *mech, BattleMap *mech_map, int weaponnum,
                   int weapontype, int sight, int section, int critical);
