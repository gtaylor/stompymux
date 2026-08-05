#pragma once

#include <stdbool.h>

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

bool battle_map_unit_is_seen(const BattleMap *map, const Mech *observer,
                             const Mech *target);
bool battle_map_unit_los_is_blocked(const BattleMap *map, const Mech *observer,
                                    const Mech *target);
int battle_map_unit_los_wood_count(const BattleMap *map, const Mech *observer,
                                   const Mech *target);
int battle_map_unit_los_water_count(const BattleMap *map, const Mech *observer,
                                    const Mech *target);
unsigned short battle_map_los_flags(const BattleMap *map, int observer_index,
                                    int target_index);
void battle_map_los_flags_set(BattleMap *map, int observer_index,
                              int target_index, unsigned short flags);
void battle_map_los_observer_clear(BattleMap *map, int observer_index);
