#include "aero_move_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "ds_bay_api.h"
#include "map.h"
#include "map_los.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_map_render_internal.h"
#include "mech_maps_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mine.h"
#include "mux/server/game.h"
#include "registry_api.h"

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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
  int elevation = map_elevation_get(map, x, y);
  char terrain = map_real_terrain_get(map, x, y);
  return terrain == WATER || terrain == ICE ? -elevation : elevation;
}

/*
 * Draw one of the seven hexes that a Dropship takes up on a tac map.
 */
static void sketch_tac_ds(char *base, int dispcols, char terr) {
  /*
   * Becareful not to overlay a 'mech id or terrain elevation.
   */
  if (!isalpha(base[0]) && base[0] != '*') {
    base[0] = terr;
    base[1] = terr;
  }
  base[dispcols + 0] = terr;
  if (!isdigit((unsigned char)base[dispcols + 1])) {
    base[dispcols + 1] = terr;
  }
}

extern const int dirs[6][2];

static void sketch_tac_ownmech(char *buf, BattleMap *map, Mech *mech, int sx,
                               int sy, int wx, int wy, int dispcols,
                               int top_offset, int left_offset) {

  int oddcol1 = tactical_column_is_odd(sx);
  char *pos = buf + top_offset * dispcols + left_offset;
  char *base;
  int x = mech_position_x(mech) - sx;
  int y = mech_position_y(mech) - sy;

  if (x < 0 || x >= wx || y < 0 || y >= wy) {
    return;
  }
  base = pos + tactical_hex_offset(x, y, dispcols, oddcol1);
  base[0] = '*';
  base[0] = '*';
}

static void sketch_tac_mechs(char *buf, BattleMap *map, Mech *player_mech,
                             int sx, int sy, int wx, int wy, int dispcols,
                             int top_offset, int left_offset, int docolour,
                             int labels) {
  int i;
  char *pos = buf + top_offset * dispcols + left_offset;
  int oddcol1 = tactical_column_is_odd(sx);

  /*
   * Draw all the 'mechs on the map.
   */
  for (i = 0; i < map->first_free; i++) {
    int x, y;
    char *base;
    Mech *mech;

    if (map->mechsOnMap[i] == -1) {
      continue;
    }

    mech = btech_context_get_mech(map->xcode.context, map->mechsOnMap[i]);
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

    base = pos + tactical_hex_offset(x, y, dispcols, oddcol1);
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
        int tx = x + dirs[dir][0];
        int ty = y + dirs[dir][1];

        if ((tx + oddcol1) % 2 == 0 && dirs[dir][0] != 0) {
          ty--;
        }
        if (tx < 0 || tx >= wx || ty < 0 || ty >= wy) {
          continue;
        }
        base = pos + tactical_hex_offset(tx, ty, dispcols, oddcol1);
        if (dropship_bay_number(mech, (dir - ts + 6) % 6) >= 0) {
          sketch_tac_ds(base, dispcols, '@');
        } else {
          sketch_tac_ds(base, dispcols, '=');
        }
      }
      if (x < 0 || x >= wx || y < 0 || y >= wy)
        continue;

      base = pos + tactical_hex_offset(x, y, dispcols, oddcol1);
      if (docolour) {
        /*
         * Colour hack: 'X' would be confused with
         * any enemy contact by style_tac_map()
         */
        sketch_tac_ds(base, dispcols, '$');
      } else {
        sketch_tac_ds(base, dispcols, 'X');
      }

      if (isalpha(base[0]))
        continue;

      if (mech == player_mech) {
        base[0] = '*';
        base[1] = '*';
      } else {
        MechId id = mech_id(mech, mech_seems_friendly(player_mech, mech));
        base[0] = id.text[0];
        base[1] = id.text[1];
      }

    } else if (mech == player_mech) {
      if (isalpha(base[0]))
        continue;
      base[0] = '*';
      base[1] = '*';
    } else {
      MechId id = mech_id(mech, mech_seems_friendly(player_mech, mech));
      base[0] = id.text[0];
      base[1] = id.text[1];
    }
  }
}

static void sketch_tac_cliffs(char *buf, BattleMap *map, int sx, int sy, int wx,
                              int wy, int dispcols, int top_offset,
                              int left_offset, int cliff_size) {
  char *pos = buf + top_offset * dispcols + left_offset;
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
      char *base = pos + tactical_hex_offset(x, y, dispcols, oddcol1);
      char c;

      /*
       * Copy the elevation up to the top of the hex
       * so we can draw a bottom hex edge on every hex.
       */
      c = base[dispcols + 1];
      if (base[0] == '*') {
        base[0] = '*';
        base[1] = '*';
      } else if (isdigit((unsigned char)c)) {
        base[1] = c;
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

        base[dispcols - 1] = '|';
      }
      if (y < wy - 1 &&
          abs(map_base_elevation(map, tx, ty + 1) - elev) >= cliff_size) {
        base[dispcols] = ',';
        base[dispcols + 1] = ',';
      } else {
        base[dispcols] = '_';
        base[dispcols + 1] = '_';
      }
      if (x < wx - 1 && (y < wy - 1 || oddcolx) &&
          abs(map_base_elevation(map, tx + 1, ty + 1 - oddcolx) - elev) >=
              cliff_size) {
        base[dispcols + 2] = '!';
      }
    }
  }
}
static void sketch_tac_dslz(char *buf, BattleMap *map, Mech *mech, int sx,
                            int sy, int wx, int wy, int dispcols,
                            int top_offset, int left_offset, int cliff_size,
                            int docolour) {
  char *pos = buf + top_offset * dispcols + left_offset;
  int y, x;
  int oddcol1 = tactical_column_is_odd(sx);

  wx = minimum_int(wx, map->map_width - sx);
  wy = minimum_int(wy, map->map_height - sy);
  for (y = maximum_int(0, -sy); y < wy; y++) {
    int ty = sy + y;

    for (x = maximum_int(0, -sx); x < wx; x++) {
      int tx = sx + x;
      char *base = pos + tactical_hex_offset(x, y, dispcols, oddcol1);

      if (aero_landing_zone_check(mech, tx, ty))
        base[dispcols] = docolour ? '\241' : 'X';
      else
        base[dispcols] = docolour ? '\240' : 'O';
    }
  }
}

static void sketch_tac_mines(char *buf, BattleMap *map, Mech *mech, int sx,
                             int sy, int wx, int wy, int dispcols,
                             int top_offset, int left_offset) {
  char *pos = buf + top_offset * dispcols + left_offset;
  int y, x;
  int oddcol1 = tactical_column_is_odd(sx);
  float fx, fy, fz, hex_range;
  MapObject *o;

  wx = minimum_int(wx, map->map_width - sx);
  wy = minimum_int(wy, map->map_height - sy);
  for (y = maximum_int(0, -sy); y < wy; y++) {
    int ty = sy + y;
    for (x = maximum_int(0, -sx); x < wx; x++) {
      int tx = sx + x;
      char *base = pos + tactical_hex_offset(x, y, dispcols, oddcol1);
      char c;

      /*
       * Copy the elevation up to the top of the hex
       * so we can draw a bottom hex edge on every hex.
       */
      c = base[dispcols + 1];
      if (base[0] == '*') {
        base[0] = '*';
        base[1] = '*';
      } else if (isdigit((unsigned char)c)) {
        base[1] = c;
      }

      /*
       * For each hex on the map check to see if there is
       * a mine. If so and we see the hex, we see the mine.
       * Mines are shown with '<>'.  Optionally, if
       * commented code is used the mine strength, but
       * not type, is displayed in the bottom of the hex.
       * Hide triggers so they can be used unbeknownst to players
       */
      for (o = map->MapObject[TYPE_MINE]; o; o = o->next)
        if (o->x == tx && o->y == ty)
          break;
      base[dispcols] = ' ';
      base[dispcols + 1] = ' ';
      if (o) {
        int elevation;

        MapCoordToRealCoord(tx, ty, &fx, &fy);
        elevation = map_base_elevation(map, tx, ty);
        fz = ZSCALE * (float)elevation;
        hex_range =
            FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                      mech_position_real_z(mech), fx, fy, fz);
        if ((o->datac != MINE_TRIGGER) &&
            mech_los_check_unblocked(mech, nullptr, tx, ty, hex_range)) {
          /*     base[dispcols]=(o->datas/10) + '0'; */
          /*     base[dispcols+1]=(o->datas%10) + '0'; */
          base[dispcols] = '<';
          base[dispcols + 1] = '>';
        }
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
  text->lines = calloc(line_capacity, sizeof(*text->lines));
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

void map_text_destroy(MapText *text) {
  if (text == nullptr)
    return;
  free(text->buffer);
  free(text->lines);
  free(text);
}

MapText *map_text_create(DbRef player, Mech *mech, BattleMap *map, int cx,
                         int cy, int wx, int wy, int labels, int dohexlos) {
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
  char *base;
  char *sketch_buf;
  int oddcol1;
  enum {
    MAX_WIDTH = 40,
    MAX_HEIGHT = 24,
    TOP_LABEL = 3,
    LEFT_LABEL = 4,
    RIGHT_LABEL = 3,
    MAP_SKETCH_CAPACITY = ((LEFT_LABEL + 1 + MAX_WIDTH * 3 + RIGHT_LABEL + 1) *
                               (TOP_LABEL + 1 + MAX_HEIGHT * 2) +
                           2) *
                          5,
  };

  map_color_scheme_load(&colors);

  if (labels & 4) {
    navigate = 1;
    labels = 0;
  }

  /*
   * Figure out the extent of the tac map to draw.
   */
  wx = minimum_int(MAX_WIDTH, wx);
  wy = minimum_int(MAX_HEIGHT, wy);
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
      left_offset = LEFT_LABEL;
      dispcols += LEFT_LABEL + RIGHT_LABEL;
    }
    if (labels & 2) {
      top_offset = TOP_LABEL;
      disprows += TOP_LABEL;
    }
  }

  sketch_buf = calloc(MAP_SKETCH_CAPACITY, sizeof(*sketch_buf));
  if (sketch_buf == nullptr)
    return nullptr;

  /*
   * Create a sketch tac map including terrain and elevation.
   */
  tactical_map_sketch(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                      top_offset, left_offset, docolour, dohexlos,
                      dounderlying);

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
      snprintf(scratch, sizeof(scratch), "%3d", label);
      base = sketch_buf + left_offset + 1 + x * 3;
      base[0] = scratch[0];
      base[1 * dispcols] = scratch[1];
      base[2 * dispcols] = scratch[2];
    }
  }

  if (labels & 2) {
    int y;

    for (y = 0; y < wy; y++) {
      int label = sy + y;
      size_t row_offset;
      size_t right_label_offset;

      row_offset = (size_t)(top_offset + 1 + y * 2) * (size_t)dispcols;
      right_label_offset = row_offset + (size_t)(dispcols - RIGHT_LABEL - 1);
      base = sketch_buf + row_offset;
      if (label < 0 || label > 999) {
        continue;
      }

      snprintf(base, MAP_SKETCH_CAPACITY - row_offset, "%3d", label);
      base[3] = ' ';
      snprintf(sketch_buf + right_label_offset,
               MAP_SKETCH_CAPACITY - right_label_offset, "%3d", label);
    }
  }

  if (labels & 8) {
    if (mech != nullptr) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
    }
    sketch_tac_cliffs(sketch_buf, map, sx, sy, wx, wy, dispcols, top_offset,
                      left_offset, 3);
  } else if (labels & 16) {
    if (mech != nullptr) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
    }
    sketch_tac_cliffs(sketch_buf, map, sx, sy, wx, wy, dispcols, top_offset,
                      left_offset, 2);
  } else if (labels & 32) {
    if (mech != nullptr) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
    }
    sketch_tac_dslz(sketch_buf, map, mech, sx, sy, wx, wy, dispcols, top_offset,
                    left_offset, 2, docolour);
  } else if (labels & 128) {
    if (mech != nullptr) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
      sketch_tac_mines(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                       top_offset, left_offset);
      sketch_tac_mechs(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                       top_offset, left_offset, docolour, labels);
    }

  } else if (mech != nullptr) {
    sketch_tac_mechs(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                     top_offset, left_offset, docolour, labels);
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

      base = sketch_buf + (i + 1) * dispcols + left_offset;
      len = (n - i - 1) * 3 + 1;
      memset(base, ' ', (size_t)len);
      base[len] = '_';
      base[len + 1] = '_';
      base[mapcols - len - 2] = '_';
      base[mapcols - len - 1] = '_';
      base[mapcols - len] = '\0';

      base = sketch_buf + (disprows - i - 1) * dispcols + left_offset;
      len = (n - i) * 3;
      memset(base, ' ', (size_t)len);
      base[mapcols - len] = '\0';
    }

    memset(sketch_buf + left_offset, ' ', (size_t)(n * 3 + 1));
    sketch_buf[left_offset + n * 3 + 1] = '_';
    sketch_buf[left_offset + n * 3 + 2] = '_';
    sketch_buf[left_offset + n * 3 + 3] = '\0';
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
    text->lines[i] = text->buffer + dispcols * i;
  }
  text->lines[i] = nullptr;
  return text;
}

/* Draws the map for the player when they use the
 * TACTICAL [C | T | L] [<BEARING> <RANGE> | <TARGET-ID>]
 * command inside a unit */
