#include "mech_maps_internal.h"

static inline int is_oddcol(int col) {
  /*
   * The only real trick here is to handle negative
   * numbers correctly.
   */
  return (unsigned)col & 1;
}

static inline int tac_dispcols(int hexcols) { return hexcols * 3 + 1; }

static inline int tac_hex_offset(int x, int y, int dispcols, int oddcol1) {
  int oddcolx = is_oddcol(x + oddcol1);

  return (y * 2 + 1 - oddcolx) * dispcols + x * 3 + 1;
}

static inline void sketch_tac_row(char *pos, int left_offset, char const *src,
                                  int len) {
  memset(pos, ' ', left_offset);
  memcpy(pos + left_offset, src, len);
  pos[left_offset + len] = '\0';
}

static void sketch_tac_map(char *buf, BattleMap *map, Mech *mech, int sx,
                           int sy, int wx, int wy, int dispcols, int top_offset,
                           int left_offset, int docolour, int dohexlos,
                           int dounderlying)

{
#if 0
	static char const hexrow[2][76] = {
		"\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/",
		"/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\][/][\\"
	};
#else
  static char const hexrow[2][310] = {
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
#endif
  int x, y;
  int oddcol1 = is_oddcol(sx); /* One iff first hex col is odd */
  char *pos;
  int mapcols = tac_dispcols(wx);
  HexLosMap los_map_storage;
  HexLosMap *losmap = nullptr;

  /*
   * First create a blank hex map.
   */
  pos = buf;
  for (y = 0; y < top_offset; y++) {
    memset(pos, ' ', dispcols - 1);
    pos[dispcols - 1] = '\0';
    pos += dispcols;
  }
  for (y = 0; y < wy; y++) {
    sketch_tac_row(pos, left_offset, hexrow[oddcol1], mapcols);
    pos += dispcols;
    sketch_tac_row(pos, left_offset, hexrow[!oddcol1], mapcols);
    pos += dispcols;
  }
  sketch_tac_row(pos, left_offset, hexrow[oddcol1], mapcols);

  /*
   * Now draw the terrain and elevation.
   */
  pos = buf + top_offset * dispcols + left_offset;
  wx = MIN(wx, map->map_width - sx);
  wy = MIN(wy, map->map_height - sy);

  if (dohexlos && los_map_calculate(&los_map_storage, map, mech, MAX(0, sx),
                                    MAX(0, sy), wx, wy))
    losmap = &los_map_storage;

  for (y = MAX(0, -sy); y < wy; y++) {
    for (x = MAX(0, -sx); x < wx; x++) {
      int terr, elev, rterr, losflag = MAPLOSHEX_SEE | MAPLOSHEX_SEEN;
      char *base;
      char topchar, botchar;

      if (losmap)
        losflag = LOS_MAP_GET_FLAG(losmap, sx + x, sy + y);

      if (!(losflag & MAPLOSHEX_SEEN)) {
        terr = 'X';
        elev = 40; /* 'X' */
      } else {

        if (losflag & MAPLOSHEX_SEETERRAIN)
          terr = map_terrain_get(map, sx + x, sy + y);
        else
          terr = UNKNOWN_TERRAIN;

        if (losflag & MAPLOSHEX_SEEELEV)
          elev = map_elevation_get(map, sx + x, sy + y);
        else
          elev = 15; /* Ugly hack: '0' + 15 == '?' */
      }
      base = pos + tac_hex_offset(x, y, dispcols, oddcol1);

      switch (terr) {
      case WATER:
        /*
         * Colour hack:  Draw deep water with '\242'
         * if using colour so style_tac_map()
         * knows to use dark blue rather than light
         * blue
         */
        if (docolour && elev >= 2) {
          topchar = '\242';
          botchar = '\242';
        } else {
          topchar = '~';
          botchar = '~';
        }
        break;

      case SMOKE:
      case FIRE:
        if (dounderlying) {
          rterr = map_real_terrain_get(map, sx + x, sy + y);
          topchar = terr;
          botchar = rterr;
        } else {
          topchar = terr;
          botchar = terr;
        }
        break;

      case HIGHWATER:
        topchar = '~';
        botchar = '+';
        break;

      case BRIDGE:
        topchar = '#';
        botchar = '+';
        break;

      case ' ': /* GRASSLAND */
        topchar = ' ';
        botchar = '_';
        break;

      case UNKNOWN_TERRAIN:
        topchar = '?';
        botchar = '?';
        break;

      default:
        topchar = terr;
        botchar = terr;
        break;
      }

      base[0] = topchar;
      base[1] = topchar;
      base[dispcols + 0] = botchar;
      if (elev > 0) {
        botchar = '0' + elev;
      }
      base[dispcols + 1] = botchar;
    }
  }
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

  int oddcol1 = is_oddcol(sx);
  char *pos = buf + top_offset * dispcols + left_offset;
  char *base;
  int x = MechX(mech) - sx;
  int y = MechY(mech) - sy;

  if (x < 0 || x >= wx || y < 0 || y >= wy) {
    return;
  }
  base = pos + tac_hex_offset(x, y, dispcols, oddcol1);
  base[0] = '*';
  base[0] = '*';
}

static void sketch_tac_mechs(char *buf, BattleMap *map, Mech *player_mech,
                             int sx, int sy, int wx, int wy, int dispcols,
                             int top_offset, int left_offset, int docolour,
                             int labels) {
  int i;
  char *pos = buf + top_offset * dispcols + left_offset;
  int oddcol1 = is_oddcol(sx);

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
    if (mech == NULL) {
      continue;
    }

    /*
     * Check to see if the 'mech is on the tac map and
     * that its in LOS of the player's 'mech.
     */
    x = MechX(mech) - sx;
    y = MechY(mech) - sy;
    if (!IsDS(mech) && (x < 0 || x >= wx || y < 0 || y >= wy)) {
      continue;
    }

    if (IsDS(mech) && (x < -1 || x > wx || y < -1 || y > wy)) {
      continue;
    }

    if (mech != player_mech &&
        !InLineOfSight(player_mech, mech, MechX(mech), MechY(mech),
                       FlMechRange(map, player_mech, mech))) {
      continue;
    }

    base = pos + tac_hex_offset(x, y, dispcols, oddcol1);
    if (!(MechSpecials2(mech) & CARRIER_TECH) && IsDS(mech) &&
        ((MechZ(mech) >= ORBIT_Z && mech != player_mech) || Landed(mech) ||
         !Started(mech))) {
      int ts = DSBearMod(mech);
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
        base = pos + tac_hex_offset(tx, ty, dispcols, oddcol1);
        if (Find_DS_Bay_Number(mech, (dir - ts + 6) % 6) >= 0) {
          sketch_tac_ds(base, dispcols, '@');
        } else {
          sketch_tac_ds(base, dispcols, '=');
        }
      }
      if (x < 0 || x >= wx || y < 0 || y >= wy)
        continue;

      base = pos + tac_hex_offset(x, y, dispcols, oddcol1);
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
        MechId id = mech_id(mech, MechSeemsFriend(player_mech, mech));
        base[0] = id.text[0];
        base[1] = id.text[1];
      }

    } else if (mech == player_mech) {
      if (isalpha(base[0]))
        continue;
      base[0] = '*';
      base[1] = '*';
    } else {
      MechId id = mech_id(mech, MechSeemsFriend(player_mech, mech));
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
  int oddcol1 = is_oddcol(sx);

  wx = MIN(wx, map->map_width - sx);
  wy = MIN(wy, map->map_height - sy);
  for (y = MAX(0, -sy); y < wy; y++) {
    int ty = sy + y;

    for (x = MAX(0, -sx); x < wx; x++) {
      int tx = sx + x;
      int oddcolx = is_oddcol(tx);
      int elev = Elevation(map, tx, ty);
      char *base = pos + tac_hex_offset(x, y, dispcols, oddcol1);
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
          abs(Elevation(map, tx - 1, ty + 1 - oddcolx) - elev) >= cliff_size) {

        base[dispcols - 1] = '|';
      }
      if (y < wy - 1 && abs(Elevation(map, tx, ty + 1) - elev) >= cliff_size) {
        base[dispcols] = ',';
        base[dispcols + 1] = ',';
      } else {
        base[dispcols] = '_';
        base[dispcols + 1] = '_';
      }
      if (x < wx - 1 && (y < wy - 1 || oddcolx) &&
          abs(Elevation(map, tx + 1, ty + 1 - oddcolx) - elev) >= cliff_size) {
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
  int oddcol1 = is_oddcol(sx);

  wx = MIN(wx, map->map_width - sx);
  wy = MIN(wy, map->map_height - sy);
  for (y = MAX(0, -sy); y < wy; y++) {
    int ty = sy + y;

    for (x = MAX(0, -sx); x < wx; x++) {
      int tx = sx + x;
      char *base = pos + tac_hex_offset(x, y, dispcols, oddcol1);

      if (ImproperLZ(mech, tx, ty))
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
  int oddcol1 = is_oddcol(sx);
  float fx, fy, fz, hex_range;
  MapObject *o;

  wx = MIN(wx, map->map_width - sx);
  wy = MIN(wy, map->map_height - sy);
  for (y = MAX(0, -sy); y < wy; y++) {
    int ty = sy + y;
    for (x = MAX(0, -sx); x < wx; x++) {
      int tx = sx + x;
      char *base = pos + tac_hex_offset(x, y, dispcols, oddcol1);
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
        MapCoordToRealCoord(tx, ty, &fx, &fy);
        fz = ZSCALE * Elevation(map, tx, ty);
        hex_range =
            FindRange(MechFX(mech), MechFY(mech), MechFZ(mech), fx, fy, fz);
        if ((o->datac != MINE_TRIGGER) &&
            InLineOfSight_NB(mech, NULL, tx, ty, hex_range)) {
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

  map_color_scheme_load(&colors, map->xcode.context, player);

  if (labels & 4) {
    navigate = 1;
    labels = 0;
  }

  /*
   * Figure out the extent of the tac map to draw.
   */
  wx = MIN(MAX_WIDTH, wx);
  wy = MIN(MAX_HEIGHT, wy);

  sx = cx - wx / 2;
  sy = cy - wy / 2;
  if (!navigate) {
    /*
     * Only allow navigate maps to include off map hexes.
     */
    sx = MAX(0, MIN(sx, map->map_width - wx));
    sy = MAX(0, MIN(sy, map->map_height - wy));
    wx = MIN(wx, map->map_width);
    wy = MIN(wy, map->map_height);
  }

  mapcols = tac_dispcols(wx);
  dispcols = mapcols + 1;
  disprows = wy * 2 + 1;
  oddcol1 = is_oddcol(sx);

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
  sketch_tac_map(sketch_buf, map, mech, sx, sy, wx, wy, dispcols, top_offset,
                 left_offset, docolour, dohexlos, dounderlying);

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
    if (mech != NULL) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
    }
    sketch_tac_cliffs(sketch_buf, map, sx, sy, wx, wy, dispcols, top_offset,
                      left_offset, 3);
  } else if (labels & 16) {
    if (mech != NULL) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
    }
    sketch_tac_cliffs(sketch_buf, map, sx, sy, wx, wy, dispcols, top_offset,
                      left_offset, 2);
  } else if (labels & 32) {
    if (mech != NULL) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
    }
    sketch_tac_dslz(sketch_buf, map, mech, sx, sy, wx, wy, dispcols, top_offset,
                    left_offset, 2, docolour);
  } else if (labels & 128) {
    if (mech != NULL) {
      sketch_tac_ownmech(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                         top_offset, left_offset);
      sketch_tac_mines(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                       top_offset, left_offset);
      sketch_tac_mechs(sketch_buf, map, mech, sx, sy, wx, wy, dispcols,
                       top_offset, left_offset, docolour, labels);
    }

  } else if (mech != NULL) {
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
      memset(base, ' ', len);
      base[len] = '_';
      base[len + 1] = '_';
      base[mapcols - len - 2] = '_';
      base[mapcols - len - 1] = '_';
      base[mapcols - len] = '\0';

      base = sketch_buf + (disprows - i - 1) * dispcols + left_offset;
      len = (n - i) * 3;
      memset(base, ' ', len);
      base[mapcols - len] = '\0';
    }

    memset(sketch_buf + left_offset, ' ', n * 3 + 1);
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
