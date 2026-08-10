
#pragma once

#include "map_coordinates.h"
#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

void mine_field_add(BattleMap *map, int x, int y, int damage);
void mine_field_trigger(Mech *mech, MineTriggerReason reason);
void mine_field_possibly_remove(Mech *mech, int x, int y);
void mine_fields_recalculate(BattleMap *map);
void mine_command_add(DbRef player, void *data, char *buffer);
void mine_command_detonate(Mech *mech, int channel);
typedef struct MineFieldScanRequest {
  DbRef player;
  Mech *mech;
  float range;
  MapHexPosition position;
} MineFieldScanRequest;

void mine_field_scan(const MineFieldScanRequest *request);
