#pragma once

#include <stdbool.h>

typedef struct BattleMap BattleMap;

bool battle_map_building_is_invisible(const BattleMap *map);
bool battle_map_building_is_hidden(const BattleMap *map);
bool battle_map_building_is_safe(const BattleMap *map);
bool battle_map_building_is_command_center(const BattleMap *map);
int battle_map_building_integrity(const BattleMap *map);
int battle_map_building_maximum_integrity(const BattleMap *map);
