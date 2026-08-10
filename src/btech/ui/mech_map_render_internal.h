#pragma once

#include "map_coordinates.h"
#include "mech_api_types.h"

#include <stdbool.h>
#include <stddef.h>

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

constexpr char DEFAULT_COLOR_SCHEME[] = "BbWXYyRWWWXGgbRHYR\0GR";

typedef struct MapColorScheme {
  char values[NUM_COLOR_IDX + 1];
} MapColorScheme;

typedef struct MapCellText {
  char text[32];
} MapCellText;

typedef struct TerrainColorRequest {
  const MapColorScheme *colors;
  char terrain;
  int elevation;
} TerrainColorRequest;

typedef struct MapText {
  char *buffer;
  char **lines;
  size_t buffer_capacity;
  size_t line_capacity;
} MapText;

void map_color_scheme_load(MapColorScheme *colors);
char map_terrain_color_char(const TerrainColorRequest *request);
const char *map_color_markup(char color);
bool style_tac_map(MapText *text, const MapColorScheme *colors,
                   const char *sketch, int display_columns, int display_rows);
int tactical_column_is_odd(int column);
int tactical_display_columns(int hex_columns);
typedef struct TacticalHexOffsetRequest {
  MapHexPosition position;
  int display_columns;
  bool first_column_is_odd;
} TacticalHexOffsetRequest;
int tactical_hex_offset(const TacticalHexOffsetRequest *request);
void tactical_map_sketch(char *buffer, size_t buffer_capacity, BattleMap *map,
                         Mech *mech, int start_x, int start_y, int width,
                         int height, int display_columns, int top_offset,
                         int left_offset, bool use_color, bool use_hex_los,
                         bool show_underlying_terrain);
typedef struct TacticalArgumentParseRequest {
  DbRef player;
  Mech *mech;
  char *const *arguments;
  size_t argument_capacity;
  size_t first_argument;
  int argument_count;
  int maximum_range;
} TacticalArgumentParseRequest;

typedef struct TacticalArgumentParseResult {
  bool valid;
  MapHexPosition position;
} TacticalArgumentParseResult;

TacticalArgumentParseResult
tactical_arguments_parse(const TacticalArgumentParseRequest *request);
