#pragma once

#include <stdbool.h>

#include "mux/objects/db.h"

typedef struct BattleMap BattleMap;
typedef struct BtechContext BtechContext;

enum { BATTLE_MAP_UNIT_CAPACITY = 250 };

int battle_map_unit_count(const BattleMap *map);
DbRef battle_map_unit_dbref(const BattleMap *map, int index);
DbRef battle_map_dbref(const BattleMap *map);
BtechContext *battle_map_context(const BattleMap *map);
void battle_map_unit_slot_clear(BattleMap *map, int index);
void battle_map_unit_moved_flags_clear(BattleMap *map);
int battle_map_width(const BattleMap *map);
int battle_map_height(const BattleMap *map);
DbRef battle_map_parent_dbref(const BattleMap *map);
bool battle_map_blocks_friendly_fire(const BattleMap *map);
bool battle_map_is_combat_safe(const BattleMap *map);
void battle_map_parent_dbref_set(BattleMap *map, DbRef parent);
