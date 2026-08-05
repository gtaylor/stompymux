
#pragma once

#include "map.h"
#include "mech.h"
#include "mux/server/platform.h"

void add_mine(BattleMap *map, int x, int y, int dam);
void make_mine_explode(Mech *mech, BattleMap *map, MapObject *o, int x, int y,
                       int reason);
void possible_mine_poof(Mech *mech, int reason);
void possibly_remove_mines(Mech *mech, int x, int y);
void recalculate_minefields(BattleMap *map);
void map_add_mine(DbRef player, void *data, char *buffer);
void explode_mines(Mech *mech, int chn);
void show_mines_in_hex(DbRef player, Mech *mech, float range, int x, int y);
