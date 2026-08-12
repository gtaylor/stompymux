#include "mech_map_render_internal.h"

#include "map.h"
#include "map_los.h"
#include "map_terrain.h"
#include "mux/support/checked_storage.h"

#include <stdlib.h>
#include <string.h>

typedef struct TacticalCanvas {
  char *data;
  size_t capacity;
  int display_columns;
} TacticalCanvas;

static char *tactical_canvas_at(TacticalCanvas *canvas, int offset) {
  if (offset < 0)
    abort();
  return checked_storage_at(canvas->data, canvas->capacity, sizeof(char),
                            (size_t)offset);
}

static void *tactical_canvas_region(TacticalCanvas *canvas, int offset,
                                    int length) {
  if (offset < 0 || length < 0)
    abort();
  return checked_storage_region(canvas->data, canvas->capacity, (size_t)offset,
                                (size_t)length);
}

static int minimum_int(int left, int right) {
  return left < right ? left : right;
}

static int maximum_int(int left, int right) {
  return left > right ? left : right;
}

int tactical_column_is_odd(int column) { return (unsigned)column & 1; }

int tactical_display_columns(int hex_columns) { return hex_columns * 3 + 1; }

int tactical_hex_offset(const TacticalHexOffsetRequest *request) {
  const int X = request->position.x;
  const int Y = request->position.y;
  int column_is_odd = tactical_column_is_odd(X + request->first_column_is_odd);
  return (Y * 2 + 1 - column_is_odd) * request->display_columns + X * 3 + 1;
}

static void tactical_row_sketch(TacticalCanvas *canvas, int row_offset,
                                int left_offset, const char *source,
                                int length) {
  if (row_offset < 0 || left_offset < 0 || length < 0)
    abort();
  memset(tactical_canvas_region(canvas, row_offset, left_offset), ' ',
         (size_t)left_offset);
  memcpy(tactical_canvas_region(canvas, row_offset + left_offset, length),
         source, (size_t)length);
  *tactical_canvas_at(canvas, row_offset + left_offset + length) = '\0';
}

void tactical_map_sketch(char *buffer, size_t buffer_capacity, BattleMap *map,
                         Mech *mech, int start_x, int start_y, int width,
                         int height, int display_columns, int top_offset,
                         int left_offset, bool use_color, bool use_hex_los,
                         bool show_underlying_terrain) {
  static const char HEX_ROWS[2][311] = {
      "\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\][/]["
      "\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\][/]["
      "\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\][/]["
      "\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\][/]["
      "\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/",
      "/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\]["
      "/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\]["
      "/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\]["
      "/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/"
      "][\\]["
      "/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\"};
  int first_column_is_odd = tactical_column_is_odd(start_x);
  int map_columns = tactical_display_columns(width);
  HexLosMap los_map_storage;
  HexLosMap *los_map = nullptr;
  TacticalCanvas canvas = {
      .data = buffer,
      .capacity = buffer_capacity,
      .display_columns = display_columns,
  };

  for (int y = 0; y < top_offset; y++) {
    const int ROW_OFFSET = y * display_columns;
    memset(tactical_canvas_region(&canvas, ROW_OFFSET, display_columns - 1),
           ' ', (size_t)(display_columns - 1));
    *tactical_canvas_at(&canvas, ROW_OFFSET + display_columns - 1) = '\0';
  }
  for (int y = 0; y < height; y++) {
    const char (*first_row)[311] = checked_storage_at_const(
        HEX_ROWS, 2, sizeof(*HEX_ROWS), (size_t)first_column_is_odd);
    const char (*second_row)[311] = checked_storage_at_const(
        HEX_ROWS, 2, sizeof(*HEX_ROWS), (size_t)!first_column_is_odd);
    int row_offset = (top_offset + y * 2) * display_columns;
    tactical_row_sketch(&canvas, row_offset, left_offset, *first_row,
                        map_columns);
    row_offset += display_columns;
    tactical_row_sketch(&canvas, row_offset, left_offset, *second_row,
                        map_columns);
  }
  const char (*last_row)[311] = checked_storage_at_const(
      HEX_ROWS, 2, sizeof(*HEX_ROWS), (size_t)first_column_is_odd);
  tactical_row_sketch(&canvas, (top_offset + height * 2) * display_columns,
                      left_offset, *last_row, map_columns);

  const int MAP_ORIGIN_OFFSET = top_offset * display_columns + left_offset;
  width = minimum_int(width, map->map_width - start_x);
  height = minimum_int(height, map->map_height - start_y);
  if (use_hex_los &&
      los_map_calculate(&los_map_storage, map, mech, maximum_int(0, start_x),
                        maximum_int(0, start_y), width, height))
    los_map = &los_map_storage;

  for (int y = maximum_int(0, -start_y); y < height; y++) {
    for (int x = maximum_int(0, -start_x); x < width; x++) {
      int terrain;
      int elevation;
      int los_flags = MAPLOSHEX_SEE | MAPLOSHEX_SEEN;
      if (los_map)
        los_flags = los_map_flag(los_map, start_x + x, start_y + y);

      if (!(los_flags & MAPLOSHEX_SEEN)) {
        terrain = 'X';
        elevation = 40;
      } else {
        terrain = los_flags & MAPLOSHEX_SEETERRAIN
                      ? map_terrain_get(map, start_x + x, start_y + y)
                      : UNKNOWN_TERRAIN;
        elevation = los_flags & MAPLOSHEX_SEEELEV
                        ? map_elevation_get(map, start_x + x, start_y + y)
                        : 15;
      }

      char top_character;
      char bottom_character;
      switch (terrain) {
      case WATER:
        top_character = bottom_character =
            use_color && elevation >= 2 ? '\242' : '~';
        break;
      case SMOKE:
      case FIRE:
        top_character = (char)terrain;
        bottom_character =
            show_underlying_terrain
                ? map_real_terrain_get(map, start_x + x, start_y + y)
                : (char)terrain;
        break;
      case HIGHWATER:
        top_character = '~';
        bottom_character = '+';
        break;
      case BRIDGE:
        top_character = '#';
        bottom_character = '+';
        break;
      case ' ':
        top_character = ' ';
        bottom_character = '_';
        break;
      case UNKNOWN_TERRAIN:
        top_character = bottom_character = '?';
        break;
      default:
        top_character = bottom_character = (char)terrain;
        break;
      }

      const int BASE_OFFSET =
          MAP_ORIGIN_OFFSET + tactical_hex_offset(&(TacticalHexOffsetRequest){
                                  .position = {.x = x, .y = y},
                                  .display_columns = display_columns,
                                  .first_column_is_odd = first_column_is_odd});
      *tactical_canvas_at(&canvas, BASE_OFFSET) = top_character;
      *tactical_canvas_at(&canvas, BASE_OFFSET + 1) = top_character;
      *tactical_canvas_at(&canvas, BASE_OFFSET + display_columns) =
          bottom_character;
      if (elevation > 0)
        bottom_character = (char)('0' + elevation);
      *tactical_canvas_at(&canvas, BASE_OFFSET + display_columns + 1) =
          bottom_character;
    }
  }
}
