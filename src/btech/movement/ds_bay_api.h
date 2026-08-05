#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;

void mech_createbays(DbRef player, void *data, char *buffer);
int dropship_bay_number(Mech *ds, int direction);
int dropship_bay_direction(Mech *ds, int bay);
int dropship_bay_in_adjacent_hex(Mech *seer, Mech *dropship, long *bay);
void mech_enterbay(DbRef player, void *data, char *buffer);
int dropship_leave(BattleMap *map, Mech *mech);
