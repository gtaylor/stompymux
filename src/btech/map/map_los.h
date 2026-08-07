
/*
 * $Id: map.los.h,v 1.1.1.1 2005/01/11 21:18:08 kstevens Exp $
 *
 * Author: Thomas Wouters <thomas@xs4all.net>
 *
 *  Copyright (c) 2002 Thomas Wouters
 *      All rights reserved
 *
 */

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

int los_map_hex_index(HexLosMap *los_map, int x, int y);

static inline unsigned char los_map_flag(const HexLosMap *los_map, int x,
                                         int y) {
  return los_map->map[los_map_hex_index((HexLosMap *)los_map, x, y)];
}

bool los_map_calculate(HexLosMap *los_map, BattleMap *map, Mech *mech,
                       int start_x, int start_y, int x_size, int y_size);
