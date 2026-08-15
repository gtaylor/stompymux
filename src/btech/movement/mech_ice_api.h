/* Declares the BattleTech unit ice API. */

#pragma once

#include "map_coordinates.h"
#include "mux/server/platform.h"

typedef struct TerrainStructureWeaponImpact {
  Mech *attacker;
  int weapon_index;
  MapHexPosition position;
} TerrainStructureWeaponImpact;

/* mech.ice.c */
void drop_thru_ice(Mech *mech);
void break_thru_ice(Mech *mech);
bool possibly_drop_thru_ice(Mech *mech);
bool growable(BattleMap *map, int x, int y);
bool meltable(BattleMap *map, int x, int y);
void ice_growth(DbRef player, BattleMap *map, int num);
void ice_melt(DbRef player, BattleMap *map, int num);
void map_addice(DbRef player, BattleMap *map, char *buffer);
void map_delice(DbRef player, BattleMap *map, char *buffer);
void ice_weapon_impact_resolve(const TerrainStructureWeaponImpact *impact);
void bridge_weapon_impact_resolve(const TerrainStructureWeaponImpact *impact);
