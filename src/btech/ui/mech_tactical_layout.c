#include "mech_map_render_internal.h"

#include "map.h"
#include "map_los.h"
#include "map_terrain.h"

#include <string.h>

static int minimum_int(int left, int right) {
  return left < right ? left : right;
}

static int maximum_int(int left, int right) {
  return left > right ? left : right;
}

int tactical_column_is_odd(int column) { return (unsigned)column & 1; }

int tactical_display_columns(int hex_columns) { return hex_columns * 3 + 1; }

int tactical_hex_offset(int x, int y, int display_columns,
                        int first_column_is_odd) {
  int column_is_odd = tactical_column_is_odd(x + first_column_is_odd);
  return (y * 2 + 1 - column_is_odd) * display_columns + x * 3 + 1;
}

static void tactical_row_sketch(char *position, int left_offset,
                                const char *source, int length) {
  memset(position, ' ', left_offset);
  memcpy(position + left_offset, source, length);
  position[left_offset + length] = '\0';
}

void tactical_map_sketch(char *buffer, BattleMap *map, Mech *mech, int start_x,
                         int start_y, int width, int height,
                         int display_columns, int top_offset, int left_offset,
                         bool use_color, bool use_hex_los,
                         bool show_underlying_terrain) {
  static const char hex_rows[2][310] = {
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
  char *position = buffer;

  for (int y = 0; y < top_offset; y++) {
    memset(position, ' ', display_columns - 1);
    position[display_columns - 1] = '\0';
    position += display_columns;
  }
  for (int y = 0; y < height; y++) {
    tactical_row_sketch(position, left_offset, hex_rows[first_column_is_odd],
                        map_columns);
    position += display_columns;
    tactical_row_sketch(position, left_offset, hex_rows[!first_column_is_odd],
                        map_columns);
    position += display_columns;
  }
  tactical_row_sketch(position, left_offset, hex_rows[first_column_is_odd],
                      map_columns);

  position = buffer + top_offset * display_columns + left_offset;
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
        top_character = terrain;
        bottom_character =
            show_underlying_terrain
                ? map_real_terrain_get(map, start_x + x, start_y + y)
                : terrain;
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
        top_character = bottom_character = terrain;
        break;
      }

      char *base = position + tactical_hex_offset(x, y, display_columns,
                                                  first_column_is_odd);
      base[0] = top_character;
      base[1] = top_character;
      base[display_columns] = bottom_character;
      if (elevation > 0)
        bottom_character = '0' + elevation;
      base[display_columns + 1] = bottom_character;
    }
  }
}
