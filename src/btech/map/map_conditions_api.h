/* Declares the BattleTech map conditions API. */

#pragma once

#include <stddef.h>
#include <stdio.h>

#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

/* map.conditions.c */
void alter_conditions(BattleMap *map);
void map_setconditions(DbRef player, BattleMap *map, char *buffer);
void map_conditions_apply(Mech *mech, BattleMap *map);
bool map_read_dimensions(FILE *file, int *width, int *height);
bool map_parse_visibility_attribute(const char *attribute, int *visibility,
                                    int *light, int *wind_direction,
                                    int *wind_speed, int *cloud_base,
                                    char *message, size_t message_size);
int battle_map_gravity(const BattleMap *map);
int battle_map_light(const BattleMap *map);
int battle_map_visibility(const BattleMap *map);
int battle_map_maximum_visibility(const BattleMap *map);
int battle_map_cloud_base(const BattleMap *map);
int battle_map_temperature(const BattleMap *map);
float battle_map_movement_modifier(const BattleMap *map);
bool battle_map_is_dark(const BattleMap *map);
bool battle_map_is_underground(const BattleMap *map);
bool battle_map_uses_special_rules(const BattleMap *map);
bool battle_map_sensor_is_disabled(const BattleMap *map, int sensor);
bool battle_map_bridges_have_capacity(const BattleMap *map);
bool battle_map_is_vacuum(const BattleMap *map);
bool battle_map_disables_bridgification(const BattleMap *map);
bool battle_map_disables_friendly_fire(const BattleMap *map);
bool battle_map_disables_physicals(const BattleMap *map);
bool battle_map_build_is_complex(const BattleMap *map);
bool battle_map_build_is_complex_structure(const BattleMap *map);
bool battle_map_build_is_hidden(const BattleMap *map);
bool battle_map_build_is_safe(const BattleMap *map);
bool battle_map_build_is_invisible(const BattleMap *map);
bool battle_map_build_is_dropship_structure(const BattleMap *map);
