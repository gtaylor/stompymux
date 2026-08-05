
/*
   p.map.conditions.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:42 CET 1999 from map.conditions.c */

#pragma once

#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

/* map.conditions.c */
void alter_conditions(BattleMap *map);
void map_setconditions(DbRef player, BattleMap *map, char *buffer);
void map_conditions_apply(Mech *mech, BattleMap *map);
int battle_map_gravity(const BattleMap *map);
int battle_map_light(const BattleMap *map);
int battle_map_visibility(const BattleMap *map);
int battle_map_maximum_visibility(const BattleMap *map);
int battle_map_cloud_base(const BattleMap *map);
int battle_map_temperature(const BattleMap *map);
bool battle_map_is_dark(const BattleMap *map);
bool battle_map_is_underground(const BattleMap *map);
bool battle_map_uses_special_rules(const BattleMap *map);
bool battle_map_sensor_is_disabled(const BattleMap *map, int sensor);
bool battle_map_bridges_have_capacity(const BattleMap *map);
