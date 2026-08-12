#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
#include "map_los.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_map_render_internal.h"
#include "mech_maps_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

#include "mux/support/formatting.h"
#include "section_types.h"
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct LrsMechList {
  Mech *items[MAX_MECHS_PER_MAP + 1];
} LrsMechList;

static Mech **lrs_mech_slot(LrsMechList *list, int index) {
  return (Mech **)checked_storage_at((void *)list->items, MAX_MECHS_PER_MAP + 1,
                                     sizeof(*list->items), (size_t)index);
}

static Mech *lrs_mech_at(const LrsMechList *list, int index) {
  return *(Mech *const *)checked_storage_at_const(
      (const void *)list->items, MAX_MECHS_PER_MAP + 1, sizeof(*list->items),
      (size_t)index);
}

static void lrs_text_append(char *buffer, size_t capacity, const char *format,
                            ...) __attribute__((format(printf, 3, 4)));

static void lrs_text_append(char *buffer, size_t capacity, const char *format,
                            ...) {
  const size_t USED = strlen(buffer);
  if (USED >= capacity)
    return;
  va_list arguments;
  va_start(arguments, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(
      checked_storage_region(buffer, capacity, USED, capacity - USED),
      capacity - USED, format, arguments);
  va_end(arguments);
}

static bool mech_seems_friendly(Mech *mech, Mech *other) {
  return mech_team(mech) == mech_team(other) &&
         mech_los_check_unblocked(mech, other, 0, 0, 0);
}

char get_lrs_mech_char(Mech *mech, Mech *other) {
  char c = 'u';

  if (mech == other)
    return '*';
  if (mech_is_dropship(other))
    c = 'd';
  switch (mech_movement_type(other)) {
  case MOVE_FLY:
    c = 'a';
    break;
  case MOVE_BIPED:
    c = 'b';
    break;
  case MOVE_QUAD:
    c = 'q';
    break;
  case MOVE_TRACK:
    c = 't';
    break;
  case MOVE_WHEEL:
    c = 'w';
    break;
  case MOVE_HOVER:
    c = 'h';
    break;
  case MOVE_VTOL:
    c = 'v';
    break;
  case MOVE_HULL:
    c = 'n';
    break;
  case MOVE_SUB:
    c = 's';
    break;
  case MOVE_FOIL:
    c = 'f';
    break;
  case MOVE_NONE:
    break;
  }
  if (!mech_seems_friendly(mech, other))
    c = ascii_to_upper(c);
  return c;
}

char map_terrain_color_char(const TerrainColorRequest *request) {
  const MapColorScheme *colors = request->colors;

  switch (request->terrain) {
  case HIGHWATER:
    return colors->values[DWATER_IDX];
  case WATER:
    if (request->elevation < 2 || request->elevation == '0' ||
        request->elevation == '1' || request->elevation == '~')
      return colors->values[SWATER_IDX];
    return colors->values[DWATER_IDX];
  case BUILDING:
    return colors->values[BUILDING_IDX];
  case ROAD:
    return colors->values[ROAD_IDX];
  case DESERT:
  case ROUGH:
    return colors->values[ROUGH_IDX];
  case MOUNTAINS:
    return colors->values[MOUNTAIN_IDX];
  case FIRE:
    return colors->values[FIRE_IDX];
  case ICE:
    return colors->values[ICE_IDX];
  case WALL:
    return colors->values[WALL_IDX];
  case SNOW:
    return colors->values[SNOW_IDX];
  case SMOKE:
    return colors->values[SMOKE_IDX];
  case LIGHT_FOREST:
    return colors->values[LWOOD_IDX];
  case HEAVY_FOREST:
    return colors->values[HWOOD_IDX];
  case UNKNOWN_TERRAIN:
    return colors->values[UNKNOWN_IDX];
  }
  return '\0';
}

const char *map_color_markup(char color) {
  bool bold = color >= 'A' && color <= 'Z';
  const char NORMALIZED = bold ? (char)(color + ('a' - 'A')) : color;

  switch (NORMALIZED) {
  case 'x':
    return bold ? "[fg=black bold]" : "[fg=black]";
  case 'r':
    return bold ? "[fg=red bold]" : "[fg=red]";
  case 'g':
    return bold ? "[fg=green bold]" : "[fg=green]";
  case 'y':
    return bold ? "[fg=yellow bold]" : "[fg=yellow]";
  case 'b':
    return bold ? "[fg=blue bold]" : "[fg=blue]";
  case 'm':
    return bold ? "[fg=magenta bold]" : "[fg=magenta]";
  case 'c':
    return bold ? "[fg=cyan bold]" : "[fg=cyan]";
  case 'w':
    return bold ? "[fg=white bold]" : "[fg=white]";
  case 'f':
    return bold ? "[blink bold]" : "[blink]";
  case 'i':
    return bold ? "[inverse bold]" : "[inverse]";
  case 'h':
    return "[bold]";
  default:
    return "";
  }
}

static MapCellText map_cell_text(char newc, char *prevc, char c) {
  MapCellText result = {0};

  if (newc == *prevc) {
    result.text[0] = c;
    return result;
  }

  (void)snprintf(result.text, sizeof(result.text), "[reset]%s%c",
                 map_color_markup(newc), c);
  *prevc = newc;
  return result;
}

static MapCellText lrs_mech_text(const MapColorScheme *colors, Mech *mech,
                                 Mech *other, int docolor, char *prevc) {
  char c = get_lrs_mech_char(mech, other);
  char newc;

  if (!docolor) {
    MapCellText result = {0};
    result.text[0] = c;
    return result;
  }

  if (mech == other)
    newc = colors->values[SELF_IDX];
  else if (!mech_seems_friendly(mech, other))
    newc = colors->values[ENEMY_IDX];
  else
    newc = colors->values[FRIEND_IDX];

  return map_cell_text(newc, prevc, c);
}

typedef struct LrsCellRequest {
  const MapColorScheme *colors;
  BattleMap *map;
  MapHexPosition position;
  bool use_color;
  char *previous_color;
} LrsCellRequest;

static MapCellText lrs_terrain_text(const LrsCellRequest *request) {
  char c =
      map_terrain_get(request->map, request->position.x, request->position.y);
  char newc;

  if (!c || !request->use_color || c == ' ') {
    MapCellText result = {0};
    result.text[0] = c;
    return result;
  }
  newc = map_terrain_color_char(&(TerrainColorRequest){
      .colors = request->colors,
      .terrain = c,
      .elevation = map_elevation_get(request->map, request->position.x,
                                     request->position.y)});

  return map_cell_text(newc, request->previous_color, c);
}

static MapCellText lrs_elevation_text(const LrsCellRequest *request) {
  int e = (unsigned char)map_elevation_get(request->map, request->position.x,
                                           request->position.y);
  char c;
  char newc;

  if (!e && !request->use_color)
    c = ' ';
  else if (e >= 0 && e <= 9)
    c = (char)('0' + e);
  else
    c = '?';

  if (!request->use_color) {
    MapCellText result = {0};
    result.text[0] = c;
    return result;
  }
  newc = map_terrain_color_char(&(TerrainColorRequest){
      .colors = request->colors,
      .terrain = map_terrain_get(request->map, request->position.x,
                                 request->position.y),
      .elevation = e});

  return map_cell_text(newc, request->previous_color, c);
}

#define LRS_TERRAINMODE 1
#define LRS_ELEVMODE 2
#define LRS_MECHMODE 4
#define LRS_LOSMODE 8
#define LRS_COLORMODE 16
#define LRS_ELEVCOLORMODE 32

static MapCellText lrs_hex_text(const MapColorScheme *colors, Mech *mech,
                                BattleMap *map, int x, int y, char *prevc,
                                int mode, const LrsMechList *mechs, int lm,
                                HexLosMap *losmap) {
  int losflag = MAPLOSHEX_SEE | MAPLOSHEX_SEEN;

  if (mode & LRS_MECHMODE) {
    while (lrs_mech_at(mechs, lm) &&
           mech_position_y(lrs_mech_at(mechs, lm)) < y)
      lm++;
    while (lrs_mech_at(mechs, lm) &&
           mech_position_y(lrs_mech_at(mechs, lm)) == y &&
           mech_position_x(lrs_mech_at(mechs, lm)) < x)
      lm++;
    if (lrs_mech_at(mechs, lm) &&
        mech_position_y(lrs_mech_at(mechs, lm)) == y &&
        mech_position_x(lrs_mech_at(mechs, lm)) == x)
      return lrs_mech_text(colors, mech, lrs_mech_at(mechs, lm),
                           mode & LRS_COLORMODE, prevc);
  }

  if (losmap)
    losflag = los_map_flag(losmap, x, y);

  /* If the losmap doesn't contain this hex, we return X in bold red
   * in both terrain and elevation mode.
   */
  if (!(losflag & MAPLOSHEX_SEEN))
    return map_cell_text('R', prevc, 'X');

  if (((mode & LRS_TERRAINMODE) && !(losflag & MAPLOSHEX_SEETERRAIN)) ||
      ((mode & LRS_ELEVMODE) && !(losflag & MAPLOSHEX_SEEELEV)))
    return map_cell_text(
        map_terrain_color_char(&(TerrainColorRequest){
            .colors = colors, .terrain = UNKNOWN_TERRAIN, .elevation = 0}),
        prevc, '?');

  if (mode & LRS_ELEVMODE)
    return lrs_elevation_text(
        &(LrsCellRequest){.colors = colors,
                          .map = map,
                          .position = {.x = x, .y = y},
                          .use_color = (mode & LRS_ELEVCOLORMODE) != 0,
                          .previous_color = prevc});
  if (mode & LRS_TERRAINMODE)
    return lrs_terrain_text(
        &(LrsCellRequest){.colors = colors,
                          .map = map,
                          .position = {.x = x, .y = y},
                          .use_color = (mode & LRS_COLORMODE) != 0,
                          .previous_color = prevc});

  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                     tprintf("Unknown LRS mode, mech #%ld mode 0x%x.",
                             mech_dbref(mech), mode));
  return map_cell_text('R', prevc, 'Y');
}

typedef struct LrsMapRequest {
  const MapColorScheme *colors;
  DbRef player;
  Mech *mech;
  BattleMap *map;
  MapHexPosition center;
  int display_height;
  int mode;
} LrsMapRequest;

static void show_lrs_map(const LrsMapRequest *request) {
  const MapColorScheme *colors = request->colors;
  const DbRef PLAYER = request->player;
  Mech *mech = request->mech;
  BattleMap *map = request->map;
  const int X = request->center.x;
  const int Y = request->center.y;
  const int DISPLAY_HEIGHT = request->display_height;
  const int MODE = request->mode;
  int loop, b_width, e_width, b_height, e_height, i;
  Mech *o_mech;

  /* These buffers hold styled map cells and their coordinate labels. */
  char topbuff[LBUF_SIZE] = "    ";
  char botbuff[LBUF_SIZE] = "    ";
  char midbuff[LBUF_SIZE] = "    ";
  char trash1[16]; /* temp var to hold the map coordinate label */
  short oddcol = 0;
  LrsMechList mechs = {0};
  int last_mech = 0;
  char prevct = 0, prevcb = 0;
  HexLosMap los_map_storage;
  HexLosMap *losmap = nullptr;

  /* x and y hold the viewing center of the map */
  b_width = X - LRS_DISPLAY_WIDTH / 2;
  b_width = max(b_width, 0);
  e_width = b_width + LRS_DISPLAY_WIDTH;
  if (e_width >= map->map_width) {
    e_width = map->map_width - 1;
    b_width = e_width - LRS_DISPLAY_WIDTH;
    b_width = max(b_width, 0);
  }

  if (b_width % 2)
    oddcol = 1;

  b_height = Y - DISPLAY_HEIGHT / 2;
  b_height = max(b_height, 0);
  e_height = b_height + DISPLAY_HEIGHT;
  if (e_height > map->map_height) {
    e_height = map->map_height;
    b_height = e_height - DISPLAY_HEIGHT;
    b_height = max(b_height, 0);
  }

  /* Display the top labels */
  for (i = b_width; i <= e_width; i++) {
    (void)snprintf(trash1, sizeof(trash1), "%3d", i);
    lrs_text_append(topbuff, sizeof(topbuff), "%c", *trash1);
    lrs_text_append(midbuff, sizeof(midbuff), "%c",
                    *checked_string_suffix(trash1, 1));
    lrs_text_append(botbuff, sizeof(botbuff), "%c",
                    *checked_string_suffix(trash1, 2));
  }
  mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER, topbuff);
  mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER, midbuff);
  mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER, botbuff);

  if (MODE & LRS_MECHMODE) {
    for (i = 0; i < battle_map_unit_count(map); i++) {
      o_mech = btech_context_get_mech(mech_context(mech),
                                      battle_map_unit_dbref(map, i));
      if (o_mech) {
        if ((mech == o_mech) ||
            (mech_position_y(o_mech) >= b_height &&
             mech_position_y(o_mech) <= e_height &&
             mech_position_x(o_mech) >= b_width &&
             mech_position_x(o_mech) <= e_width &&
             mech_los_check(mech, o_mech, mech_position_x(o_mech),
                            mech_position_y(o_mech),
                            mech_range_to(mech, o_mech))))
          *lrs_mech_slot(&mechs, last_mech++) = o_mech;
      }
    }
    for (i = 0; i < (last_mech - 1); i++) /* Bubble-sort the list
                                           *  to y/x order */
      for (loop = (i + 1); loop < last_mech; loop++) {
        if (mech_position_y(lrs_mech_at(&mechs, i)) >
            mech_position_y(lrs_mech_at(&mechs, loop))) {
          o_mech = lrs_mech_at(&mechs, i);
          *lrs_mech_slot(&mechs, i) = lrs_mech_at(&mechs, loop);
          *lrs_mech_slot(&mechs, loop) = o_mech;
        } else if (mech_position_y(lrs_mech_at(&mechs, i)) ==
                       mech_position_y(lrs_mech_at(&mechs, loop)) &&
                   mech_position_x(lrs_mech_at(&mechs, i)) >
                       mech_position_x(lrs_mech_at(&mechs, loop))) {
          o_mech = lrs_mech_at(&mechs, i);
          *lrs_mech_slot(&mechs, i) = lrs_mech_at(&mechs, loop);
          *lrs_mech_slot(&mechs, loop) = o_mech;
        }
      }
    *lrs_mech_slot(&mechs, last_mech) = nullptr;
    last_mech = 0;
  }

  if ((MODE & LRS_LOSMODE) &&
      los_map_calculate(&los_map_storage, map, mech, b_width, b_height,
                        e_width - b_width, e_height - b_height))
    losmap = &los_map_storage;

  for (loop = b_height; loop < e_height; loop++) {
    (void)snprintf(topbuff, sizeof(topbuff), "%3d ", loop);
    strcpy(botbuff, "    ");
    if (MODE & LRS_MECHMODE)
      while (lrs_mech_at(&mechs, last_mech) &&
             mech_position_y(lrs_mech_at(&mechs, last_mech)) < loop)
        last_mech++;

    for (i = b_width; i < e_width; i += 2) {
      lrs_text_append(topbuff, sizeof(topbuff), oddcol ? "%s " : " %s",
                      lrs_hex_text(colors, mech, map, i + !oddcol, loop,
                                   &prevct, MODE, &mechs, last_mech, losmap)
                          .text);

      lrs_text_append(botbuff, sizeof(botbuff), oddcol ? " %s" : "%s ",
                      lrs_hex_text(colors, mech, map, i + oddcol, loop, &prevcb,
                                   MODE, &mechs, last_mech, losmap)
                          .text);
    }
    if (i == e_width && !oddcol) {
      lrs_text_append(botbuff, sizeof(botbuff), "%s",
                      lrs_hex_text(colors, mech, map, i, loop, &prevcb, MODE,
                                   &mechs, last_mech, losmap)
                          .text);
    } else if (i == e_width) {
      lrs_text_append(topbuff, sizeof(topbuff), "%s",
                      lrs_hex_text(colors, mech, map, i, loop, &prevct, MODE,
                                   &mechs, last_mech, losmap)
                          .text);
      strlcat(botbuff, " ", sizeof(botbuff));
    }

    if (MODE & (LRS_COLORMODE | LRS_ELEVCOLORMODE)) {
      if (prevct) {
        strlcat(topbuff, "[reset]", sizeof(topbuff));
        prevct = 0;
      }
      if (prevcb) {
        strlcat(botbuff, "[reset]", sizeof(botbuff));
        prevcb = 0;
      }
    }
    lrs_text_append(botbuff, sizeof(botbuff), " %-3d", loop);
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER, topbuff);
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER, botbuff);
  }
}

void mech_lrsmap(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  MapColorScheme colors;
  BattleMap *map;
  int argc, mode = 0;
  int x, y;
  char *args[5], *str;
  int display_height = LRS_DISPLAY_HEIGHT;

  if (!common_checks(player, mech, MECH_USUAL))
    return;

  if (is_ansi(mech_context(mech)->database, player))
    mode |= LRS_COLORMODE;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  argc = mech_parseattributes(buffer, args, 4);
  if (!mech_long_range_sensor_range(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your system seems to be inoperational.");
    return;
  }
  const TacticalArgumentParseResult PARSED =
      tactical_arguments_parse(&(TacticalArgumentParseRequest){
          .player = player,
          .mech = mech,
          .arguments = args,
          .argument_capacity = 5,
          .first_argument = 1,
          .argument_count = argc - 1,
          .maximum_range = mech_long_range_sensor_range(mech),
      });
  if (!PARSED.valid)
    return;
  x = PARSED.position.x;
  y = PARSED.position.y;
  switch (args[0][0]) {
  case 'M':
  case 'm':
    mode |= LRS_MECHMODE | LRS_TERRAINMODE;
    break;
  case 'E':
  case 'e':
    mode |= LRS_ELEVMODE;
    break;
  case 'C':
  case 'c':
    mode |= LRS_ELEVMODE | LRS_ELEVCOLORMODE;
    break;
  case 'T':
  case 't':
    mode |= LRS_TERRAINMODE;
    break;
  case 'L':
  case 'l':
    mode |= LRS_LOSMODE | LRS_TERRAINMODE;
    break;
  case 'H':
  case 'h':
    mode |= LRS_LOSMODE | LRS_ELEVMODE;
    break;
  case 'S':
  case 's':
    mode |= LRS_LOSMODE | LRS_MECHMODE | LRS_TERRAINMODE;
    break;
  default:
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "Unknown LRS sensor type '%s'!", args[0]);
    return;
  }

  if (battle_map_is_dark(map) ||
      (mech_class(mech) == CLASS_MW &&
       mech_context(mech)->configuration->btech_mw_losmap))
    mode |= LRS_LOSMODE;

  str = btech_attribute_read(mech_context(mech)->database, player, A_LRSHEIGHT,
                             (char[LBUF_SIZE]){0});
  if (*str) {
    if (!parse_int_checked(str, &display_height) || display_height < 10 ||
        display_height > 40) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Illegal LRSHeight attribute.  Must be between 10 and 40");
      display_height = LRS_DISPLAY_HEIGHT;
    }
  }

  display_height = min(display_height, 2 * mech_long_range_sensor_range(mech));
  display_height = min(display_height, map->map_height);

  if (!(display_height % 2))
    display_height++;

  map_color_scheme_load(&colors);

  show_lrs_map(&(LrsMapRequest){.colors = &colors,
                                .player = player,
                                .mech = mech,
                                .map = map,
                                .center = {.x = x, .y = y},
                                .display_height = display_height,
                                .mode = mode});
}
