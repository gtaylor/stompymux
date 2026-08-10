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
  const int elevation = (unsigned char)map_elevation_get(map, x, y);
  const char terrain = map_real_terrain_get(map, x, y);
  return terrain == WATER || terrain == ICE ? -elevation : elevation;
}

void tactical_sketch_landing_zones(const TacticalSketch *sketch) {
  const int origin_offset =
      sketch->top_offset * sketch->display_columns + sketch->left_offset;
  const int first_column_is_odd = tactical_column_is_odd(sketch->start_x);
  const int width =
      minimum_int(sketch->width, sketch->map->map_width - sketch->start_x);
  const int height =
      minimum_int(sketch->height, sketch->map->map_height - sketch->start_y);

  for (int y = maximum_int(0, -sketch->start_y); y < height; ++y) {
    const int map_y = sketch->start_y + y;
    for (int x = maximum_int(0, -sketch->start_x); x < width; ++x) {
      const int map_x = sketch->start_x + x;
      const int base_offset =
          origin_offset + tactical_hex_offset(&(TacticalHexOffsetRequest){
                              .position = {.x = x, .y = y},
                              .display_columns = sketch->display_columns,
                              .first_column_is_odd = first_column_is_odd,
                          });
      *tactical_canvas_at(sketch->canvas,
                          base_offset + sketch->display_columns) =
          aero_landing_zone_check(sketch->mech, map_x, map_y)
              ? (sketch->color ? '\241' : 'X')
              : (sketch->color ? '\240' : 'O');
    }
  }
}

void tactical_sketch_mines(const TacticalSketch *sketch) {
  const int origin_offset =
      sketch->top_offset * sketch->display_columns + sketch->left_offset;
  const int first_column_is_odd = tactical_column_is_odd(sketch->start_x);
  const int width =
      minimum_int(sketch->width, sketch->map->map_width - sketch->start_x);
  const int height =
      minimum_int(sketch->height, sketch->map->map_height - sketch->start_y);

  for (int y = maximum_int(0, -sketch->start_y); y < height; ++y) {
    const int map_y = sketch->start_y + y;
    for (int x = maximum_int(0, -sketch->start_x); x < width; ++x) {
      const int map_x = sketch->start_x + x;
      const int base_offset =
          origin_offset + tactical_hex_offset(&(TacticalHexOffsetRequest){
                              .position = {.x = x, .y = y},
                              .display_columns = sketch->display_columns,
                              .first_column_is_odd = first_column_is_odd,
                          });
      const char elevation = *tactical_canvas_at(
          sketch->canvas, base_offset + sketch->display_columns + 1);
      if (*tactical_canvas_at(sketch->canvas, base_offset) == '*') {
        *tactical_canvas_at(sketch->canvas, base_offset + 1) = '*';
      } else if (ascii_is_digit(elevation)) {
        *tactical_canvas_at(sketch->canvas, base_offset + 1) = elevation;
      }

      MapObject *mine =
          battle_map_object_first(sketch->map, BATTLE_MAP_OBJECT_MINE);
      while (mine != nullptr && (battle_map_object_x(mine) != map_x ||
                                 battle_map_object_y(mine) != map_y))
        mine = battle_map_object_next(mine);

      *tactical_canvas_at(sketch->canvas,
                          base_offset + sketch->display_columns) = ' ';
      *tactical_canvas_at(sketch->canvas,
                          base_offset + sketch->display_columns + 1) = ' ';
      if (mine == nullptr)
        continue;

      float real_x;
      float real_y;
      MapCoordToRealCoord(map_x, map_y, &real_x, &real_y);
      const int elevation_value = map_base_elevation(sketch->map, map_x, map_y);
      const float real_z = ZSCALE * (float)elevation_value;
      const float range = map_spatial_range(&(MapSpatialSegment){
          .start = {.x = mech_position_real_x(sketch->mech),
                    .y = mech_position_real_y(sketch->mech),
                    .z = mech_position_real_z(sketch->mech)},
          .end = {.x = real_x, .y = real_y, .z = real_z},
      });
      if (mine->datac != MINE_TRIGGER &&
          mech_los_check_unblocked(sketch->mech, nullptr, map_x, map_y,
                                   range)) {
        *tactical_canvas_at(sketch->canvas,
                            base_offset + sketch->display_columns) = '<';
        *tactical_canvas_at(sketch->canvas,
                            base_offset + sketch->display_columns + 1) = '>';
      }
    }
  }
}
