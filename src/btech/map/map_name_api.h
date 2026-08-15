/* Declares operations on a BattleTech map's stored name. */

#pragma once

typedef struct BattleMap BattleMap;

void battle_map_name_set(BattleMap *map, const char *name);
