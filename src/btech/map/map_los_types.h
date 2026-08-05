#pragma once

typedef enum BattleMapLosFlag {
  BATTLE_MAP_LOS_SEEN = 0x0001,
  BATTLE_MAP_LOS_SEEN_PRIMARY = 0x0002,
  BATTLE_MAP_LOS_SEEN_SECONDARY = 0x0004,
  BATTLE_MAP_LOS_TERRAIN_CALCULATED = 0x0008,
  BATTLE_MAP_LOS_MOUNTAIN = 0x0010,
  BATTLE_MAP_LOS_WOOD = 0x0020,
  BATTLE_MAP_LOS_PARTIAL_COVER = 0x1000,
  BATTLE_MAP_LOS_FIRE = 0x2000,
  BATTLE_MAP_LOS_SMOKE = 0x4000,
  BATTLE_MAP_LOS_BLOCKED = 0x8000,
} BattleMapLosFlag;

enum {
  BATTLE_MAP_LOS_MAX_WOOD = 16,
  BATTLE_MAP_LOS_WATER = 0x0200,
  BATTLE_MAP_LOS_MAX_WATER = 8,
};

static inline int battle_map_los_wood_count(int flags) {
  return (flags / BATTLE_MAP_LOS_WOOD) % BATTLE_MAP_LOS_MAX_WOOD;
}

static inline int battle_map_los_water_count(int flags) {
  return (flags / BATTLE_MAP_LOS_WATER) % BATTLE_MAP_LOS_MAX_WATER;
}
