#pragma once

#include "mux/objects/db.h"

typedef struct BattleMap BattleMap;

int battle_map_unit_count(const BattleMap *map);
DbRef battle_map_unit_dbref(const BattleMap *map, int index);
