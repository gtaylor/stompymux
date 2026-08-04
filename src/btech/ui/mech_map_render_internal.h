#pragma once

#include "mech_api_types.h"

#include <stdbool.h>
#include <stddef.h>

typedef struct BtechContext BtechContext;

enum {
  SWATER_IDX,
  DWATER_IDX,
  BUILDING_IDX,
  ROAD_IDX,
  ROUGH_IDX,
  MOUNTAIN_IDX,
  FIRE_IDX,
  ICE_IDX,
  WALL_IDX,
  SNOW_IDX,
  SMOKE_IDX,
  LWOOD_IDX,
  HWOOD_IDX,
  UNKNOWN_IDX,
  CLIFF_IDX,
  SELF_IDX,
  FRIEND_IDX,
  ENEMY_IDX,
  DS_IDX,
  GOODLZ_IDX,
  BADLZ_IDX,
  NUM_COLOR_IDX
};

typedef struct MapColorScheme {
  char values[NUM_COLOR_IDX + 1];
} MapColorScheme;

typedef struct MapCellText {
  char text[32];
} MapCellText;

typedef struct MapText {
  char *buffer;
  char **lines;
  size_t buffer_capacity;
  size_t line_capacity;
} MapText;

void map_color_scheme_load(MapColorScheme *colors, BtechContext *context,
                           DbRef player);
char map_terrain_color_char(const MapColorScheme *colors, char terrain,
                            int elevation);
const char *map_color_markup(char color);
bool style_tac_map(MapText *text, const MapColorScheme *colors,
                   const char *sketch, int display_columns, int display_rows);
int parse_tacargs(DbRef player, Mech *mech, char **args, int argc, int maxrange,
                  short *x, short *y);
