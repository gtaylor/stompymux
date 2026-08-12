#include "mech_tactical_overlay_internal.h"

#include "aero_move_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_object_query_api.h"
#include "map_terrain.h"
#include "mech_los_api.h"
#include "mech_map_render_internal.h"
#include "mech_position_api.h"
#include "mech_utils_api.h"
#include "mine.h"
#include "mux/support/checked_storage.h"
#include <stddef.h>
#include <stdlib.h>

static char *tactical_canvas_at(char *canvas, int offset) {
  if (offset < 0)
    abort();
  return checked_storage_at(canvas, MAP_SKETCH_CAPACITY, sizeof(char),
                            (size_t)offset);
}

static bool ascii_is_digit(char value) { return value >= '0' && value <= '9'; }

static int minimum_int(int left, int right) {
  return left < right ? left : right;
}

static int maximum_int(int left, int right) {
  return left > right ? left : right;
}

static int map_base_elevation(BattleMap *map, int x, int y) {
  const int ELEVATION = (unsigned char)map_elevation_get(map, x, y);
  const char TERRAIN = map_real_terrain_get(map, x, y);
  return TERRAIN == WATER || TERRAIN == ICE ? -ELEVATION : ELEVATION;
}

void tactical_sketch_landing_zones(const TacticalSketch *sketch) {
  const int ORIGIN_OFFSET =
      (sketch->top_offset * sketch->display_columns) + sketch->left_offset;
  const int FIRST_COLUMN_IS_ODD = tactical_column_is_odd(sketch->start_x);
  const int WIDTH =
      minimum_int(sketch->width, sketch->map->map_width - sketch->start_x);
  const int HEIGHT =
      minimum_int(sketch->height, sketch->map->map_height - sketch->start_y);

  for (int y = maximum_int(0, -sketch->start_y); y < HEIGHT; ++y) {
    const int MAP_Y = sketch->start_y + y;
    for (int x = maximum_int(0, -sketch->start_x); x < WIDTH; ++x) {
      const int MAP_X = sketch->start_x + x;
      const int BASE_OFFSET =
          ORIGIN_OFFSET + tactical_hex_offset(&(TacticalHexOffsetRequest){
                              .position = {.x = x, .y = y},
                              .display_columns = sketch->display_columns,
                              .first_column_is_odd = FIRST_COLUMN_IS_ODD,
                          });
      char safe_marker = sketch->color ? '\241' : 'X';
      char unsafe_marker = sketch->color ? '\240' : 'O';
      *tactical_canvas_at(sketch->canvas,
                          BASE_OFFSET + sketch->display_columns) =
          aero_landing_zone_check(sketch->mech, MAP_X, MAP_Y) ? safe_marker
                                                              : unsafe_marker;
    }
  }
}

void tactical_sketch_mines(const TacticalSketch *sketch) {
  const int ORIGIN_OFFSET =
      (sketch->top_offset * sketch->display_columns) + sketch->left_offset;
  const int FIRST_COLUMN_IS_ODD = tactical_column_is_odd(sketch->start_x);
  const int WIDTH =
      minimum_int(sketch->width, sketch->map->map_width - sketch->start_x);
  const int HEIGHT =
      minimum_int(sketch->height, sketch->map->map_height - sketch->start_y);

  for (int y = maximum_int(0, -sketch->start_y); y < HEIGHT; ++y) {
    const int MAP_Y = sketch->start_y + y;
    for (int x = maximum_int(0, -sketch->start_x); x < WIDTH; ++x) {
      const int MAP_X = sketch->start_x + x;
      const int BASE_OFFSET =
          ORIGIN_OFFSET + tactical_hex_offset(&(TacticalHexOffsetRequest){
                              .position = {.x = x, .y = y},
                              .display_columns = sketch->display_columns,
                              .first_column_is_odd = FIRST_COLUMN_IS_ODD,
                          });
      const char ELEVATION = *tactical_canvas_at(
          sketch->canvas, BASE_OFFSET + sketch->display_columns + 1);
      if (*tactical_canvas_at(sketch->canvas, BASE_OFFSET) == '*') {
        *tactical_canvas_at(sketch->canvas, BASE_OFFSET + 1) = '*';
      } else if (ascii_is_digit(ELEVATION)) {
        *tactical_canvas_at(sketch->canvas, BASE_OFFSET + 1) = ELEVATION;
      }

      MapObject *mine =
          battle_map_object_first(sketch->map, BATTLE_MAP_OBJECT_MINE);
      while (mine != nullptr && (battle_map_object_x(mine) != MAP_X ||
                                 battle_map_object_y(mine) != MAP_Y))
        mine = battle_map_object_next(mine);

      *tactical_canvas_at(sketch->canvas,
                          BASE_OFFSET + sketch->display_columns) = ' ';
      *tactical_canvas_at(sketch->canvas,
                          BASE_OFFSET + sketch->display_columns + 1) = ' ';
      if (mine == nullptr)
        continue;

      float real_x;
      float real_y;
      map_coord_to_real_coord(MAP_X, MAP_Y, &real_x, &real_y);
      const int ELEVATION_VALUE = map_base_elevation(sketch->map, MAP_X, MAP_Y);
      const float REAL_Z = ZSCALE * (float)ELEVATION_VALUE;
      const float RANGE = map_spatial_range(&(MapSpatialSegment){
          .start = {.x = mech_position_real_x(sketch->mech),
                    .y = mech_position_real_y(sketch->mech),
                    .z = mech_position_real_z(sketch->mech)},
          .end = {.x = real_x, .y = real_y, .z = REAL_Z},
      });
      if (mine->datac != MINE_TRIGGER &&
          mech_los_check_unblocked(sketch->mech, nullptr, MAP_X, MAP_Y,
                                   RANGE)) {
        *tactical_canvas_at(sketch->canvas,
                            BASE_OFFSET + sketch->display_columns) = '<';
        *tactical_canvas_at(sketch->canvas,
                            BASE_OFFSET + sketch->display_columns + 1) = '>';
      }
    }
  }
}
