/* Declares the BattleTech map bits API. */

#pragma once

#include "mux/server/platform.h"

/* map.bits.c */
void set_hex_enterable(BattleMap *map, int x, int y);
void set_hex_mine(BattleMap *map, int x, int y);
void unset_hex_enterable(BattleMap *map, int x, int y);
void unset_hex_mine(BattleMap *map, int x, int y);
int is_mine_hex(BattleMap *map, int x, int y);
int is_hangar_hex(BattleMap *map, int x, int y);
void clear_hex_bits(BattleMap *map, int bits);
int bit_size(BattleMap *map);
