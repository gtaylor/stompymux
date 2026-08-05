#pragma once

#include "mux/objects/db.h"

typedef struct BattleMap BattleMap;

enum { BATTLE_MAP_UNIT_CAPACITY = 250 };

int battle_map_unit_count(const BattleMap *map);
DbRef battle_map_unit_dbref(const BattleMap *map, int index);
DbRef battle_map_dbref(const BattleMap *map);
void battle_map_unit_slot_clear(BattleMap *map, int index);
int battle_map_width(const BattleMap *map);
int battle_map_height(const BattleMap *map);
