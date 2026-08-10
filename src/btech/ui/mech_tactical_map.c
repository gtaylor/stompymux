#include "btconfig.h"
#include "btech/context.h"
#include "ds_bay_api.h"
#include "map.h"
#include "map_los.h"
#include "map_obj_api.h"
#include "map_object_query_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_map_render_internal.h"
#include "mech_maps_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tactical_overlay_internal.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct TacticalDirection {
  int x;
  int y;
} TacticalDirection;

static const TacticalDirection TACTICAL_DIRECTIONS[] = {
    {0, -1}, {1, 0}, {1, 1}, {0, 1}, {-1, 1}, {-1, 0}};

static char *tactical_canvas_at(char *canvas, int offset) {
  if (offset < 0)
    abort();
  return checked_storage_at(canvas, MAP_SKETCH_CAPACITY, sizeof(char),
                            (size_t)offset);
}

static const TacticalDirection *tactical_direction(int direction) {
  return checked_storage_at_const(
      TACTICAL_DIRECTIONS,
      sizeof(TACTICAL_DIRECTIONS) / sizeof(*TACTICAL_DIRECTIONS),
      sizeof(*TACTICAL_DIRECTIONS), (size_t)direction);
}

static bool ascii_is_alpha(char value) {
  return (value >= 'A' && value <= 'Z') || (value >= 'a' && value <= 'z');
}

static bool ascii_is_digit(char value) { return value >= '0' && value <= '9'; }

static bool mech_seems_friendly(Mech *mech, Mech *other) {
  return mech_team(mech) == mech_team(other) &&
         mech_los_check_unblocked(mech, other, 0, 0, 0);
}

static int minimum_int(int left, int right) {
  return left < right ? left : right;
}

static int maximum_int(int left, int right) {
  return left > right ? left : right;
}

static int map_base_elevation(BattleMap *map, int x, int y) {
  int elevation = (unsigned char)map_elevation_get(map, x, y);
  char terrain = map_real_terrain_get(map, x, y);
  return terrain == WATER || terrain == ICE ? -elevation : elevation;
}

/*
 * Draw one of the seven hexes that a Dropship takes up on a tac map.
 */
static void sketch_tac_ds(char *canvas, int base_offset, int dispcols,
                          char terr) {
  /*
   * Becareful not to overlay a 'mech id or terrain elevation.
   */
  char *first = tactical_canvas_at(canvas, base_offset);
  if (!ascii_is_alpha(*first) && *first != '*') {
    *first = terr;
    *tactical_canvas_at(canvas, base_offset + 1) = terr;
  }
  *tactical_canvas_at(canvas, base_offset + dispcols) = terr;
  char *lower_right = tactical_canvas_at(canvas, base_offset + dispcols + 1);
  if (!ascii_is_digit(*lower_right)) {
    *lower_right = terr;
  }
}

static void sketch_tac_ownmech(const TacticalSketch *sketch) {
  char *buf = sketch->canvas;
  Mech *mech = sketch->mech;
  int sx = sketch->start_x;
  int sy = sketch->start_y;
  int wx = sketch->width;
  int wy = sketch->height;
  int dispcols = sketch->display_columns;
  int top_offset = sketch->top_offset;
  int left_offset = sketch->left_offset;

  int oddcol1 = tactical_column_is_odd(sx);
  const int origin_offset = top_offset * dispcols + left_offset;
  int x = mech_position_x(mech) - sx;
  int y = mech_position_y(mech) - sy;

  if (x < 0 || x >= wx || y < 0 || y >= wy) {
    return;
  }
  const int base_offset =
      origin_offset + tactical_hex_offset(&(TacticalHexOffsetRequest){
                          .position = {.x = x, .y = y},
                          .display_columns = dispcols,
                          .first_column_is_odd = oddcol1});
  *tactical_canvas_at(buf, base_offset) = '*';
}

static void sketch_tac_mechs(const TacticalSketch *sketch) {
  char *buf = sketch->canvas;
  BattleMap *map = sketch->map;
  Mech *player_mech = sketch->mech;
  int sx = sketch->start_x;
  int sy = sketch->start_y;
  int wx = sketch->width;
  int wy = sketch->height;
  int dispcols = sketch->display_columns;
  int top_offset = sketch->top_offset;
  int left_offset = sketch->left_offset;
  bool docolour = sketch->color;
  int i;
  const int origin_offset = top_offset * dispcols + left_offset;
  int oddcol1 = tactical_column_is_odd(sx);

  /*
   * Draw all the 'mechs on the map.
   */
  for (i = 0; i < battle_map_unit_count(map); i++) {
    int x, y;
    Mech *mech;

    const DbRef unit_dbref = battle_map_unit_dbref(map, i);
    if (unit_dbref == -1) {
      continue;
    }

    mech = btech_context_get_mech(map->xcode.context, unit_dbref);
    if (mech == nullptr) {
      continue;
    }

    /*
     * Check to see if the 'mech is on the tac map and
     * that its in LOS of the player's 'mech.
     */
    x = mech_position_x(mech) - sx;
    y = mech_position_y(mech) - sy;
    if (!mech_is_dropship(mech) && (x < 0 || x >= wx || y < 0 || y >= wy)) {
      continue;
    }

    if (mech_is_dropship(mech) && (x < -1 || x > wx || y < -1 || y > wy)) {
      continue;
    }

    if (mech != player_mech &&
        !mech_los_check(player_mech, mech, mech_position_x(mech),
                        mech_position_y(mech),
                        mech_range_to(player_mech, mech))) {
      continue;
    }

    int base_offset =
        origin_offset + tactical_hex_offset(&(TacticalHexOffsetRequest){
                            .position = {.x = x, .y = y},
                            .display_columns = dispcols,
                            .first_column_is_odd = oddcol1});
    if (!(mech_technology_flags_secondary(mech) & CARRIER_TECH) &&
        mech_is_dropship(mech) &&
        ((mech_position_z(mech) >= ORBIT_Z && mech != player_mech) ||
         mech_is_landed(mech) || !mech_is_started(mech))) {
      int ts = ((mech_heading_degrees(mech) + 30) / 60) % 6;
      int dir;

      /*
       * Dropships are a special case.  They take up
       * seven hexes on a tac map.  First draw the
       * center hex and then the six surronding hexes.
       */

      for (dir = 0; dir < 6; dir++) {
        const TacticalDirection *direction = tactical_direction(dir);
        const int direction_x = direction->x;
        int tx = x + direction_x;
        int ty = y + direction->y;

        if ((tx + oddcol1) % 2 == 0 && direction_x != 0) {
          ty--;
        }
        if (tx < 0 || tx >= wx || ty < 0 || ty >= wy) {
          continue;
        }
        base_offset =
            origin_offset + tactical_hex_offset(&(TacticalHexOffsetRequest){
                                .position = {.x = tx, .y = ty},
                                .display_columns = dispcols,
                                .first_column_is_odd = oddcol1});
        if (dropship_bay_number(mech, (dir - ts + 6) % 6) >= 0) {
          sketch_tac_ds(buf, base_offset, dispcols, '@');
        } else {
          sketch_tac_ds(buf, base_offset, dispcols, '=');
        }
      }
      if (x < 0 || x >= wx || y < 0 || y >= wy)
        continue;

      base_offset =
          origin_offset + tactical_hex_offset(&(TacticalHexOffsetRequest){
                              .position = {.x = x, .y = y},
                              .display_columns = dispcols,
                              .first_column_is_odd = oddcol1});
      if (docolour) {
        /*
         * Colour hack: 'X' would be confused with
         * any enemy contact by style_tac_map()
         */
        sketch_tac_ds(buf, base_offset, dispcols, '$');
      } else {
        sketch_tac_ds(buf, base_offset, dispcols, 'X');
      }

      if (ascii_is_alpha(*tactical_canvas_at(buf, base_offset)))
        continue;

      if (mech == player_mech) {
        *tactical_canvas_at(buf, base_offset) = '*';
        *tactical_canvas_at(buf, base_offset + 1) = '*';
      } else {
        MechId id = mech_id(mech, mech_seems_friendly(player_mech, mech));
        *tactical_canvas_at(buf, base_offset) = *id.text;
        *tactical_canvas_at(buf, base_offset + 1) =
            *checked_string_suffix(id.text, 1);
      }

    } else if (mech == player_mech) {
      if (ascii_is_alpha(*tactical_canvas_at(buf, base_offset)))
        continue;
      *tactical_canvas_at(buf, base_offset) = '*';
      *tactical_canvas_at(buf, base_offset + 1) = '*';
    } else {
      MechId id = mech_id(mech, mech_seems_friendly(player_mech, mech));
      *tactical_canvas_at(buf, base_offset) = *id.text;
      *tactical_canvas_at(buf, base_offset + 1) =
          *checked_string_suffix(id.text, 1);
    }
  }
}

static void sketch_tac_cliffs(const TacticalSketch *sketch) {
  char *buf = sketch->canvas;
  BattleMap *map = sketch->map;
  int sx = sketch->start_x;
  int sy = sketch->start_y;
  int wx = sketch->width;
  int wy = sketch->height;
  int dispcols = sketch->display_columns;
  int top_offset = sketch->top_offset;
  int left_offset = sketch->left_offset;
  int cliff_size = sketch->cliff_size;
  const int origin_offset = top_offset * dispcols + left_offset;
  int y, x;
  int oddcol1 = tactical_column_is_odd(sx);

  wx = minimum_int(wx, map->map_width - sx);
  wy = minimum_int(wy, map->map_height - sy);
  for (y = maximum_int(0, -sy); y < wy; y++) {
    int ty = sy + y;

    for (x = maximum_int(0, -sx); x < wx; x++) {
      int tx = sx + x;
      int oddcolx = tactical_column_is_odd(tx);
      int elev = map_base_elevation(map, tx, ty);
      const int base_offset =
          origin_offset + tactical_hex_offset(&(TacticalHexOffsetRequest){
                              .position = {.x = x, .y = y},
                              .display_columns = dispcols,
                              .first_column_is_odd = oddcol1});
      char c;

      /*
       * Copy the elevation up to the top of the hex
       * so we can draw a bottom hex edge on every hex.
       */
      c = *tactical_canvas_at(buf, base_offset + dispcols + 1);
      if (*tactical_canvas_at(buf, base_offset) == '*') {
        *tactical_canvas_at(buf, base_offset) = '*';
        *tactical_canvas_at(buf, base_offset + 1) = '*';
      } else if (ascii_is_digit(c)) {
        *tactical_canvas_at(buf, base_offset + 1) = c;
      }

      /*
       * For each hex on the map check to see if each
       * of it's 240, 180, and 120 hex sides is a cliff.
       * Don't check for cliffs between hexes that are on
       * the tac map and those that are off of it.
       */

      if (x != 0 && (y < wy - 1 || oddcolx) &&
          abs(map_base_elevation(map, tx - 1, ty + 1 - oddcolx) - elev) >=
              cliff_size) {

        *tactical_canvas_at(buf, base_offset + dispcols - 1) = '|';
      }
      if (y < wy - 1 &&
          abs(map_base_elevation(map, tx, ty + 1) - elev) >= cliff_size) {
        *tactical_canvas_at(buf, base_offset + dispcols) = ',';
        *tactical_canvas_at(buf, base_offset + dispcols + 1) = ',';
      } else {
        *tactical_canvas_at(buf, base_offset + dispcols) = '_';
        *tactical_canvas_at(buf, base_offset + dispcols + 1) = '_';
      }
      if (x < wx - 1 && (y < wy - 1 || oddcolx) &&
          abs(map_base_elevation(map, tx + 1, ty + 1 - oddcolx) - elev) >=
              cliff_size) {
        *tactical_canvas_at(buf, base_offset + dispcols + 2) = '!';
      }
    }
  }
}
static MapText *map_text_allocate(size_t buffer_capacity,
                                  size_t line_capacity) {
  MapText *text = calloc(1, sizeof(*text));
  if (text == nullptr)
    return nullptr;
  text->buffer = calloc(buffer_capacity, sizeof(*text->buffer));
  text->lines = (char **)calloc(line_capacity, sizeof(*text->lines));
  if (text->buffer == nullptr || text->lines == nullptr) {
    map_text_destroy(text);
    return nullptr;
  }
  text->buffer_capacity = buffer_capacity;
  text->line_capacity = line_capacity;
  return text;
}

char *const *map_text_lines(const MapText *text) {
  return text != nullptr ? text->lines : nullptr;
}

size_t map_text_line_count(const MapText *text) {
  return text != nullptr && text->line_capacity > 0 ? text->line_capacity - 1
                                                    : 0;
}

const char *map_text_line(const MapText *text, size_t index) {
  if (text == nullptr)
    return nullptr;
  char *const *slot = (char *const *)checked_storage_at_const(
      (const void *)text->lines, text->line_capacity, sizeof(*text->lines),
      index);
  return *slot;
}

void map_text_destroy(MapText *text) {
  if (text == nullptr)
    return;
  free(text->buffer);
  free((void *)text->lines);
  free(text);
}

MapText *map_text_create(const MapTextRequest *request) {
  DbRef player = request->player;
  Mech *mech = request->mech;
  BattleMap *map = request->map;
  int cx = request->center_x;
  int cy = request->center_y;
  int wx = request->width;
  int wy = request->height;
  int labels = request->labels;
  MapColorScheme colors;
  int docolour = is_ansi(map->xcode.context->database, player);
  int dounderlying = labels & 64;
  int dispcols;
  int disprows;
  int mapcols;
  int left_offset = 0;
  int top_offset = 0;
  int navigate = 0;
  int sx, sy;
  int i;
  char *sketch_buf;
  int oddcol1;

  map_color_scheme_load(&colors);

  if (labels & 4) {
    navigate = 1;
    labels = 0;
  }

  /*
   * Figure out the extent of the tac map to draw.
   */
  wx = minimum_int(TACTICAL_MAX_WIDTH, wx);
  wy = minimum_int(TACTICAL_MAX_HEIGHT, wy);
  if (wx <= 0 || wy <= 0)
    return nullptr;

  sx = cx - wx / 2;
  sy = cy - wy / 2;
  if (!navigate) {
    /*
     * Only allow navigate maps to include off map hexes.
     */
    sx = maximum_int(0, minimum_int(sx, map->map_width - wx));
    sy = maximum_int(0, minimum_int(sy, map->map_height - wy));
    wx = minimum_int(wx, map->map_width);
    wy = minimum_int(wy, map->map_height);
  }

  mapcols = tactical_display_columns(wx);
  dispcols = mapcols + 1;
  disprows = wy * 2 + 1;
  oddcol1 = tactical_column_is_odd(sx);

  if (navigate) {
    if (oddcol1) {
      /*
       * Insert blank line at the top where we can put
       * a "__" to make the navigate map look pretty.
       */
      top_offset = 1;
      disprows++;
    }
  } else {
    /*
     * Allow room for the labels.
     */
    if (labels & 1) {
      left_offset = TACTICAL_LEFT_LABEL;
      dispcols += TACTICAL_LEFT_LABEL + TACTICAL_RIGHT_LABEL;
    }
    if (labels & 2) {
      top_offset = TACTICAL_TOP_LABEL;
      disprows += TACTICAL_TOP_LABEL;
    }
  }

  sketch_buf = calloc(MAP_SKETCH_CAPACITY, sizeof(*sketch_buf));
  if (sketch_buf == nullptr)
    return nullptr;

  /*
   * Create a sketch tac map including terrain and elevation.
   */
  tactical_map_sketch(sketch_buf, MAP_SKETCH_CAPACITY, map, mech, sx, sy, wx,
                      wy, dispcols, top_offset, left_offset, docolour,
                      request->calculate_los, dounderlying);
  TacticalSketch sketch = {
      .canvas = sketch_buf,
      .map = map,
      .mech = mech,
      .start_x = sx,
      .start_y = sy,
      .width = wx,
      .height = wy,
      .display_columns = dispcols,
      .top_offset = top_offset,
      .left_offset = left_offset,
      .color = docolour,
      .labels = labels,
  };

  /*
   * Draw the top and side labels.
   */
  if (labels & 1) {
    int x;

    for (x = 0; x < wx; x++) {
      char scratch[4] = {0};
      int label = sx + x;

      if (label < 0 || label > 999) {
        continue;
      }
      (void)snprintf(scratch, sizeof(scratch), "%3d", label);
      const int label_offset = left_offset + 1 + x * 3;
      *tactical_canvas_at(sketch_buf, label_offset) = scratch[0];
      *tactical_canvas_at(sketch_buf, label_offset + dispcols) =
          *checked_string_suffix(scratch, 1);
      *tactical_canvas_at(sketch_buf, label_offset + 2 * dispcols) =
          *checked_string_suffix(scratch, 2);
    }
  }

  if (labels & 2) {
    int y;

    for (y = 0; y < wy; y++) {
      int label = sy + y;
      size_t row_offset;
      size_t right_label_offset;

      row_offset = (size_t)(top_offset + 1 + y * 2) * (size_t)dispcols;
      right_label_offset =
          row_offset + (size_t)(dispcols - TACTICAL_RIGHT_LABEL - 1);
      if (label < 0 || label > 999) {
        continue;
      }

      (void)snprintf(checked_storage_region(sketch_buf, MAP_SKETCH_CAPACITY,
                                            row_offset,
                                            MAP_SKETCH_CAPACITY - row_offset),
                     MAP_SKETCH_CAPACITY - row_offset, "%3d", label);
      *tactical_canvas_at(sketch_buf, (int)row_offset + 3) = ' ';
      (void)snprintf(checked_storage_region(
                         sketch_buf, MAP_SKETCH_CAPACITY, right_label_offset,
                         MAP_SKETCH_CAPACITY - right_label_offset),
                     MAP_SKETCH_CAPACITY - right_label_offset, "%3d", label);
    }
  }

  if (labels & 8) {
    if (mech != nullptr) {
      sketch_tac_ownmech(&sketch);
    }
    sketch.cliff_size = 3;
    sketch_tac_cliffs(&sketch);
  } else if (labels & 16) {
    if (mech != nullptr) {
      sketch_tac_ownmech(&sketch);
    }
    sketch.cliff_size = 2;
    sketch_tac_cliffs(&sketch);
  } else if (labels & 32) {
    if (mech != nullptr) {
      sketch_tac_ownmech(&sketch);
    }
    tactical_sketch_landing_zones(&sketch);
  } else if (labels & 128) {
    if (mech != nullptr) {
      sketch_tac_ownmech(&sketch);
      tactical_sketch_mines(&sketch);
      sketch_tac_mechs(&sketch);
    }

  } else if (mech != nullptr) {
    sketch_tac_mechs(&sketch);
  }

  if (navigate) {
    int n = wx / 2; /* Hexagon radius */

    /*
     * Navigate hack: erase characters from the sketch map
     * to turn it into a pretty hexagonal shaped map.
     */
    if (oddcol1) {
      /*
       * Don't need the last line in this case.
       */
      disprows--;
    }

    for (i = 0; i < n; i++) {
      int len;
      int base_offset;

      base_offset = (i + 1) * dispcols + left_offset;
      len = (n - i - 1) * 3 + 1;
      memset(checked_storage_region(sketch_buf, MAP_SKETCH_CAPACITY,
                                    (size_t)base_offset, (size_t)len),
             ' ', (size_t)len);
      *tactical_canvas_at(sketch_buf, base_offset + len) = '_';
      *tactical_canvas_at(sketch_buf, base_offset + len + 1) = '_';
      *tactical_canvas_at(sketch_buf, base_offset + mapcols - len - 2) = '_';
      *tactical_canvas_at(sketch_buf, base_offset + mapcols - len - 1) = '_';
      *tactical_canvas_at(sketch_buf, base_offset + mapcols - len) = '\0';

      base_offset = (disprows - i - 1) * dispcols + left_offset;
      len = (n - i) * 3;
      memset(checked_storage_region(sketch_buf, MAP_SKETCH_CAPACITY,
                                    (size_t)base_offset, (size_t)len),
             ' ', (size_t)len);
      *tactical_canvas_at(sketch_buf, base_offset + mapcols - len) = '\0';
    }

    const size_t top_blank_size = (size_t)n * 3U + 1U;
    memset(checked_storage_region(sketch_buf, MAP_SKETCH_CAPACITY,
                                  (size_t)left_offset, top_blank_size),
           ' ', top_blank_size);
    *tactical_canvas_at(sketch_buf, left_offset + n * 3 + 1) = '_';
    *tactical_canvas_at(sketch_buf, left_offset + n * 3 + 2) = '_';
    *tactical_canvas_at(sketch_buf, left_offset + n * 3 + 3) = '\0';
  }

  size_t line_capacity = (size_t)disprows + 1;
  size_t buffer_capacity = docolour ? (size_t)dispcols * (size_t)disprows * 32 +
                                          (size_t)disprows * 8 + 32
                                    : MAP_SKETCH_CAPACITY;
  MapText *text = map_text_allocate(buffer_capacity, line_capacity);
  if (text == nullptr) {
    free(sketch_buf);
    return nullptr;
  }

  if (docolour) {
    style_tac_map(text, &colors, sketch_buf, dispcols, disprows);
    free(sketch_buf);
    return text;
  }

  /*
   * If not using colour, the sketch map can be used as is.
   */
  memcpy(text->buffer, sketch_buf, MAP_SKETCH_CAPACITY);
  free(sketch_buf);
  for (i = 0; i < disprows; i++) {
    char **line_slot =
        (char **)checked_storage_at((void *)text->lines, text->line_capacity,
                                    sizeof(*text->lines), (size_t)i);
    *line_slot =
        checked_storage_at(text->buffer, text->buffer_capacity,
                           sizeof(*text->buffer), (size_t)dispcols * (size_t)i);
  }
  char **last_line =
      (char **)checked_storage_at((void *)text->lines, text->line_capacity,
                                  sizeof(*text->lines), (size_t)i);
  *last_line = nullptr;
  return text;
}

/* Draws the map for the player when they use the
 * TACTICAL [C | T | L] [<BEARING> <RANGE> | <TARGET-ID>]
 * command inside a unit */
