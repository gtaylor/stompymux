
/*
   p.map.dynamic.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:43 CET 1999 from map.dynamic.c */

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
