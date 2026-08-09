/* Declares the BattleTech map dynamic API. */

#pragma once

#include <stddef.h>

#include "mux/server/platform.h"

/* map.dynamic.c */
void battle_map_dynamic_destroy(BattleMap *map);
void mech_map_consistency_check(Mech *mech);
void eliminate_empties(BattleMap *map);
void remove_mech_from_map(BattleMap *map, Mech *mech);
void add_mech_to_map(BattleMap *newmap, Mech *mech);
size_t mech_size(const BattleMap *map);
