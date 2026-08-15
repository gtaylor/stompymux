#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;

void mech_createbays(DbRef player, void *data, char *buffer);
int dropship_bay_number(Mech *ds, int direction);
int dropship_bay_direction(Mech *ds, int num);
bool dropship_bay_in_adjacent_hex(Mech *seer, Mech *ds, int *bay);
void mech_enterbay(DbRef player, void *data, char *buffer);
bool dropship_leave(BattleMap *map, Mech *mech);
