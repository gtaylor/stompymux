#pragma once

typedef struct BattleMap BattleMap;

int map_limited_broadcast2d(BattleMap *map, float x, float y, float range,
                            const char *message);
int map_limited_broadcast3d(BattleMap *map, float x, float y, float z,
                            float range, const char *message);
