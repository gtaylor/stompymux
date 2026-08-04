#pragma once

#include <stdbool.h>

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

bool battle_map_unit_is_seen(const BattleMap *map, const Mech *observer,
                             const Mech *target);
bool battle_map_unit_los_is_blocked(const BattleMap *map, const Mech *observer,
                                    const Mech *target);
