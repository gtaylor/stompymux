/* Declares the BattleTech unit ice API. */

#pragma once

#include "mux/server/platform.h"

/* mech.ice.c */
void drop_thru_ice(Mech *mech);
void break_thru_ice(Mech *mech);
int possibly_drop_thru_ice(Mech *mech);
int growable(BattleMap *map, int x, int y);
int meltable(BattleMap *map, int x, int y);
void ice_growth(DbRef player, BattleMap *map, int num);
void ice_melt(DbRef player, BattleMap *map, int num);
void map_addice(DbRef player, BattleMap *map, char *buffer);
void map_delice(DbRef player, BattleMap *map, char *buffer);
void possibly_blow_ice(Mech *mech, int weapindx, int x, int y);
void possibly_blow_bridge(Mech *mech, int weapindx, int x, int y);
void possibly_blow_bridge(Mech *mech, int weapindx, int x, int y);
