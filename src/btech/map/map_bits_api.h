
/*
   p.map.bits.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:41 CET 1999 from map.bits.c */

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
