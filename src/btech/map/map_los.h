
/* Declares map line-of-sight interfaces. */

#pragma once

typedef struct BtechContext BtechContext;

#include "btech/context.h"
#include "mux/server/platform.h"

constexpr int MAX_SENSORS = 2;

constexpr int MAPLOS_MAXX = 70;
constexpr int MAPLOS_MAXY = 45;

constexpr int MAPLOS_FLAG_SLITE = 1;

constexpr int MAPLOSHEX_NOLOS = 0;
constexpr int MAPLOSHEX_SEEN = 1;
constexpr int MAPLOSHEX_SEETERRAIN = 2;
constexpr int MAPLOSHEX_SEEELEV = 4;
constexpr int MAPLOSHEX_LIT = 8;
#define MAPLOSHEX_SEE (MAPLOSHEX_SEETERRAIN | MAPLOSHEX_SEEELEV)

typedef struct HexLosMap {
  BtechContext *context;
  int startx;
  int starty;
  int xsize;
  int ysize;
  int flags;
  unsigned char map[MAPLOS_MAXX * MAPLOS_MAXY];
} HexLosMap;

int los_map_hex_index(const HexLosMap *map_info, int x, int y);

unsigned char los_map_flag(const HexLosMap *los_map, int x, int y);

bool los_map_calculate(HexLosMap *los_map, BattleMap *map, Mech *mech, int sx,
                       int sy, int xsz, int ysz);
