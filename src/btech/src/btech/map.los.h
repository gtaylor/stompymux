
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

#include "mux/server/platform.h"

#define MAX_SENSORS 2
#define NUMSENSORS(mech) 2

#define MAPLOS_MAXX 70
#define MAPLOS_MAXY 45

#define MAPLOS_FLAG_SLITE 1

#define MAPLOSHEX_NOLOS 0
#define MAPLOSHEX_SEEN 1
#define MAPLOSHEX_SEETERRAIN 2
#define MAPLOSHEX_SEEELEV 4
#define MAPLOSHEX_LIT 8
#define MAPLOSHEX_SEE (MAPLOSHEX_SEETERRAIN | MAPLOSHEX_SEEELEV)

#define LOS_MAP_GET_FLAG(los_map, x, y)                                        \
  ((los_map)->map[los_map_hex_index(los_map, x, y)])

typedef struct HexLosMap {
  BtechContext *context;
  int startx;
  int starty;
  int xsize;
  int ysize;
  int flags;
  unsigned char map[MAPLOS_MAXX * MAPLOS_MAXY];
} HexLosMap;

bool los_map_calculate(HexLosMap *los_map, MAP *map, MECH *mech, int start_x,
                       int start_y, int x_size, int y_size);
int los_map_hex_index(HexLosMap *los_map, int x, int y);
