
#pragma once

#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

void mine_field_add(BattleMap *map, int x, int y, int damage);
void mine_field_trigger(Mech *mech, int reason);
void mine_field_possibly_remove(Mech *mech, int x, int y);
void mine_fields_recalculate(BattleMap *map);
void mine_command_add(DbRef player, void *data, char *buffer);
void mine_command_detonate(Mech *mech, int channel);
void mine_field_scan(DbRef player, Mech *mech, float range, int x, int y);
