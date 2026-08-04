
/*
   p.mine.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:33:04 CET 1999 from mine.c */

#pragma once

#include "mux/server/platform.h"

#include "map.h"
#include "mech.h"

/* mine.c */
void add_mine(BattleMap *map, int x, int y, int dam);
void make_mine_explode(Mech *mech, BattleMap *map, MapObject *o, int x, int y,
                       int reason);
void possible_mine_poof(Mech *mech, int reason);
void possibly_remove_mines(Mech *mech, int x, int y);
void recalculate_minefields(BattleMap *map);
void map_add_mine(DbRef player, void *data, char *buffer);
void explode_mines(Mech *mech, int chn);
void show_mines_in_hex(DbRef player, Mech *mech, float range, int x, int y);
