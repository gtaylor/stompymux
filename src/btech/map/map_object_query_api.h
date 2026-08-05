#pragma once

#include "mux/objects/db.h"

typedef struct BattleMap BattleMap;
typedef struct MapObject MapObject;

enum { BATTLE_MAP_OBJECT_BUILDING = 4 };

MapObject *battle_map_object_first(BattleMap *map, int type);
MapObject *battle_map_object_next(MapObject *object);
int battle_map_object_x(const MapObject *object);
int battle_map_object_y(const MapObject *object);
DbRef battle_map_object_dbref(const MapObject *object);
