#pragma once

typedef struct BattleMap BattleMap;

int MapLimitedBroadcast2d(BattleMap *map, float x, float y, float range,
                          const char *message);
int MapLimitedBroadcast3d(BattleMap *map, float x, float y, float z,
                          float range, const char *message);
