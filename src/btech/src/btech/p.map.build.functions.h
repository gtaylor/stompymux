/* p.map.build.functions.h */

#pragma once

/* map.build.functions.c */

int water_distance(MAP *map, int x, int y, int dir, int max);
void loadMap(DbRef player, void *data, char *buffer);
void saveMap(DbRef player, void *data, char *buffer);
void freeOldMap(MAP *map);
void validateExistingLayers(MAP *map, int x, int y);
void validateSnowDepth(MAP *map, int x, int y);
void ClearTerrainLayers(MAP *map, int x, int y);
void AddTerrainLayer(MAP *map, int x, int y, int layer, int layerData);
char GetHexTerrain(MAP *map, int x, int y);
char GetHexElevation(MAP *map, int x, int y);
int GetHexLayers(MAP *map, int x, int y);
int GetHexLayerData(MAP *map, int x, int y);
void SetHexTerrain(MAP *map, int x, int y, char terrain);
void SetHexElevation(MAP *map, int x, int y, char elevation);
void SetHexLayers(MAP *map, int x, int y, int layers);
void SetHexLayerData(MAP *map, int x, int y, int layerData);
