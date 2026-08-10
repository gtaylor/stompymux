/* Declares the BattleTech map API. */

#pragma once

#include "map_coordinates.h"
#include "map_effect_types.h"
#include "mux/server/platform.h"
#include "special_object.h"

typedef struct WaterDistanceRequest {
  BattleMap *map;
  MapHexPosition origin;
  int direction;
  int limit;
} WaterDistanceRequest;

/* map.c */
void debug_fixmap(DbRef player, void *data, char *buffer);
void map_view(DbRef player, void *data, char *buffer);
void map_addhex(DbRef player, void *data, char *buffer);
void map_mapemit(DbRef player, void *data, char *buffer);
int water_distance(const WaterDistanceRequest *request);
int map_load(BattleMap *map, char *mapname);
int map_checkmapfile(BattleMap *map, char *mapname);
void map_loadmap(DbRef player, void *data, char *buffer);
void map_savemap(DbRef player, void *data, char *buffer);
void map_setmapsize(DbRef player, void *data, char *buffer);
void map_clearmechs(DbRef player, void *data, const char *buffer);
void map_update(DbRef obj, void *data);
void initialize_map_empty(BattleMap *new, DbRef key);
void newfreemap(DbRef key, void **data,
                BtechSpecialLifecycleOperation operation);
int map_sizefun(void *data, int flag);
void map_listmechs(DbRef player, void *data, char *buffer);
void clear_hex(const TerrainHexEffectRequest *request);
typedef struct MapTerrainChange {
  BattleMap *map;
  MapHexPosition position;
  int terrain;
} MapTerrainChange;
void UpdateMechsTerrain(const MapTerrainChange *change);
