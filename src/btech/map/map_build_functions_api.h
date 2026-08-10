/* p.map.build.functions.h */

#pragma once

#include "mux/server/platform.h"

/* map.build.functions.c */

void loadMap(DbRef player, void *data, char *buffer);
void saveMap(DbRef player, void *data, char *buffer);
void freeOldMap(BattleMap *map);
void validateExistingLayers(BattleMap *map, int x, int y);
void validateSnowDepth(BattleMap *map, int x, int y);
void ClearTerrainLayers(BattleMap *map, int x, int y);
void AddTerrainLayer(BattleMap *map, int x, int y, int layer, int layerData);
char GetHexTerrain(BattleMap *map, int x, int y);
char GetHexElevation(BattleMap *map, int x, int y);
int GetHexLayers(BattleMap *map, int x, int y);
int GetHexLayerData(BattleMap *map, int x, int y);
void SetHexTerrain(BattleMap *map, int x, int y, char terrain);
void SetHexElevation(BattleMap *map, int x, int y, char elevation);
void SetHexLayers(BattleMap *map, int x, int y, int layers);
void SetHexLayerData(BattleMap *map, int x, int y, int layerData);
