#include "map.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "debug_api.h"
#include "map_api.h"
#include "map_conditions_api.h"
#include "map_dynamic_api.h"
#include "map_name_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_maps_api.h"
#include "mech_notify_api.h"
#include "mech_sensor_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static char *map_argument(char **arguments, size_t count, size_t index) {
  char **slot = (char **)checked_storage_at((void *)arguments, count,
                                            sizeof(*arguments), index);
  return *slot;
}
static char *map_character(char *text, size_t capacity, size_t index) {
  return checked_storage_at(text, capacity, sizeof(*text), index);
}
static const char *map_character_const(const char *text, size_t capacity,
                                       size_t index) {
  return checked_storage_at_const(text, capacity, sizeof(*text), index);
}
static int hex_direction_x(int direction) {
  switch (direction) {
  case 0:
  case 3:
    return 0;
  case 1:
  case 2:
    return 1;
  case 4:
  case 5:
    return -1;
  default:
    abort();
  }
}
static int hex_direction_y(int direction) {
  switch (direction) {
  case 0:
    return -1;
  case 1:
  case 5:
    return 0;
  case 2:
  case 3:
  case 4:
    return 1;
  default:
    abort();
  }
}
static char *map_filename(const BattleMap *map, const char *mapname) {
  const char *map_path = map->xcode.context->configuration->database.map_db;
  char *path;
  size_t pathlen;
  pathlen = strlen(map_path) + strlen("/") + strlen(mapname) + 1;
  path = checked_storage_try_allocate(pathlen);
  if (!path)
    return nullptr;
  (void)snprintf(path, pathlen, "%s/%s", map_path, mapname);
  return path;
}
void debug_fixmap(DbRef player, void *data, char *buffer [[maybe_unused]]) {
  BattleMap *m = (BattleMap *)data;
  GameDatabase *database;
  int i;
  DbRef k;
  Mech *mek;
  if (!m)
    return;
  database = m->xcode.context->database;
  notify_printf(btech_context_evaluation(m->xcode.context), player,
                "Checking %d entries..", m->first_free);
  DOLIST(database, k, game_object_contents(database, m->mynum)) {
    if (is_xcode(database, k)) {
      if (btech_context_which_special(m->xcode.context, k) == GTYPE_MECH) {
        Mech *map_mech;
        /* Check if it's on the map */
        for (i = 0; i < battle_map_unit_count(m); i++)
          if (battle_map_unit_dbref(m, i) == k)
            break;
        if (i != battle_map_unit_count(m))
          continue;
        map_mech = btech_context_get_mech(m->xcode.context, k);
        mech_map_dbref_set(map_mech, -1); /* Eep. */
        mech_map_slot_set(map_mech, 0);
      }
    }
  }
  for (i = 0; i < battle_map_unit_count(m); i++) {
    k = battle_map_unit_dbref(m, i);
    if (k >= 0) {
      if (btech_context_which_special(m->xcode.context, k) != GTYPE_MECH) {
        notify_printf(btech_context_evaluation(m->xcode.context), player,
                      "Error: #%ld isn't mech yet is in mapindex. Fixing..", k);
        battle_map_unit_slot_clear(m, i);
      } else {
        mek = btech_context_get_mech(m->xcode.context, k);
        if (!mek) {
          notify_printf(btech_context_evaluation(m->xcode.context), player,
                        "Error: #%ld has no mech data. Removing..", k);
          battle_map_unit_slot_clear(m, i);
        } else if (mech_map_dbref(mek) != m->mynum) {
          notify_printf(btech_context_evaluation(m->xcode.context), player,
                        "Error: #%ld isn't really here! Removing..", k);
          battle_map_unit_slot_clear(m, i);
        } else if (mech_map_slot(mek) != i) {
          notify_printf(
              btech_context_evaluation(m->xcode.context), player,
              "Error: #%ld has invalid mapnumber (mn:%d <-> real:%d)..", k,
              mech_map_slot(mek), i);
        }
      }
    }
  }
  mecha_notify(btech_context_evaluation(m->xcode.context), player, "Done.");
}
/* Selectors */
/* Displays a map to player when they use the VIEW <X> <Y> command
 * with a Map Object */
void map_view(DbRef player, void *data, char *buffer) {
  BattleMap *mech_map = (BattleMap *)data;
  int argc;
  int x;
  int y;
  char *args[2];
  int display_height = MAP_DISPLAY_HEIGHT;
  int display_width = MAP_DISPLAY_WIDTH;
  char *str;
  MapText *map_text;
  /* Check if its a valid map */
  if (!mech_map)
    return;
  EvaluationContext *evaluation =
      btech_context_evaluation(mech_map->xcode.context);
  /* Make sure the proper number of arguments '<X> <Y>' were passed */
  argc = mech_parseattributes(buffer, args, 2);
  switch (argc) {
  case 2:
    if (!parse_int_checked(map_argument(args, 2, 0), &x) ||
        !parse_int_checked(map_argument(args, 2, 1), &y)) {
      mecha_notify(evaluation, player, "Invalid map coordinates!");
      return;
    }
    x = bounded(0, x, mech_map->map_width - 1);
    y = bounded(0, y, mech_map->map_height - 1);
    break;
  default:
    mecha_notify(evaluation, player, "Invalid number of parameters!");
    return;
  }
  /* Get the Tacsize attribute from
   * the player, if doesn't exist set the height and width to
   * default params. If it does exist, check the values and
   * make sure they are legit. */
  char *token_context = nullptr;
  str = btech_attribute_read(mech_map->xcode.context->database, player,
                             A_TACSIZE, (char[LBUF_SIZE]){0});
  if (!*str) {
    display_height = MAP_DISPLAY_HEIGHT;
    display_width = MAP_DISPLAY_WIDTH;
  } else if (!parse_int_checked(strtok_r(str, " \t", &token_context),
                                &display_height) ||
             !parse_int_checked(strtok_r(nullptr, " \t", &token_context),
                                &display_width) ||
             strtok_r(nullptr, " \t", &token_context) != nullptr ||
             display_height > 24 || display_height < 5 || display_width < 5 ||
             display_width > 40) {
    mecha_notify(evaluation, player,
                 "Illegal Tacsize attribute. Must be in format "
                 "'Height Width' . Height : 5-24 Width : 5-40");
    display_height = MAP_DISPLAY_HEIGHT;
    display_width = MAP_DISPLAY_WIDTH;
  }
  /* Everything worked but lets check the map size */
  display_height = (display_height <= mech_map->map_height)
                       ? display_height
                       : mech_map->map_height;
  display_width = (display_width <= mech_map->map_width) ? display_width
                                                         : mech_map->map_width;
  /* Get the map data */
  MapTextRequest request = {
      .player = player,
      .map = mech_map,
      .center_x = x,
      .center_y = y,
      .width = display_width,
      .height = display_height,
      .labels = 3,
  };
  map_text = map_text_create(&request);
  if (map_text == nullptr) {
    mecha_notify(evaluation, player, "Unable to render the tactical map.");
    return;
  }
  /* Display the map to the player */
  for (size_t line = 0; line < map_text_line_count(map_text); line++)
    mecha_notify(evaluation, player, map_text_line(map_text, line));
  map_text_destroy(map_text);
}
void map_addhex(DbRef player, void *data, char *buffer) {
  BattleMap *map;
  int x;
  int y;
  int argc;
  char *args[4];
  char elev;
  map = (BattleMap *)data;
  argc = mech_parseattributes(buffer, args, 4);
  if (argc != 4) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  int elevation;
  if (!parse_int_checked(map_argument(args, 4, 0), &x) ||
      !parse_int_checked(map_argument(args, 4, 1), &y) ||
      !parse_int_checked(map_argument(args, 4, 3), &elevation)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid numeric argument!");
    return;
  }
  elev = clamp_int_to_char(abs(elevation));
  if (!((x >= 0) && (x < map->map_width) && (y >= 0) &&
        (y < map->map_height))) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "X,Y out of range!");
    return;
  }
  char terrain = *map_character_const(map_argument(args, 4, 2),
                                      strlen(map_argument(args, 4, 2)) + 1, 0);
  if (terrain == '.')
    map_terrain_set(map, x, y, ' ');
  else
    map_terrain_set(map, x, y, terrain);
  map_elevation_set(map, x, y, (elev <= MAX_ELEV) ? elev : MAX_ELEV);
  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "Hex set!");
}
void map_mapemit(DbRef player, void *data, char *buffer) {
  BattleMap *map;
  map = (BattleMap *)data;
  buffer = map_character(buffer, strlen(buffer) + 1, strspn(buffer, " "));
  if (!buffer || !*buffer) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "What do you want to @mapemit?");
    return;
  }
  map_broadcast(map, buffer);
  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "Message sent!");
}
/* Logic: OPPOSITE sides must have water, within r<=3 of each other */
int water_distance(const WaterDistanceRequest *request) {
  BattleMap *map = request->map;
  int x = request->origin.x;
  int y = request->origin.y;
  int i;
  int x2;
  int y2;
  for (i = 1; i < request->limit; i++) {
    const int DIRECTION_X = hex_direction_x(request->direction);
    x = x + DIRECTION_X;
    y = y + hex_direction_y(request->direction);
    if (!x && DIRECTION_X)
      y--;
    x2 = bounded(0, x, map->map_width - 1);
    y2 = bounded(0, y, map->map_height - 1);
    if (x != x2 || y != y2)
      return request->limit;
    if (map_terrain_get(map, x, y) == WATER ||
        map_terrain_get(map, x, y) == ICE)
      return i;
    if (map_terrain_get(map, x, y) != BRIDGE &&
        map_terrain_get(map, x, y) != ROAD)
      return request->limit;
  }
  return request->limit;
}
static bool eligible_bridge_hex(BattleMap *map, int x, int y) {
  int i;
  int j;
  int k;
  for (k = 0; k < 3; k++) {
    i = water_distance(&(WaterDistanceRequest){
        .map = map, .origin = {.x = x, .y = y}, .direction = k, .limit = 4});
    if (i >= 4)
      continue;
    j = water_distance(&(WaterDistanceRequest){.map = map,
                                               .origin = {.x = x, .y = y},
                                               .direction = k + 3,
                                               .limit = 4});
    if (j >= 4)
      continue;
    if ((i - j) > 3)
      continue;
    return true;
  }
  return false;
}
/* Convert some of the roads to bridges */
static void make_bridges(BattleMap *map) {
  int x;
  int y;
  for (x = 0; x < map->map_width; x++)
    for (y = 0; y < map->map_height; y++)
      if (map_terrain_get(map, x, y) == ROAD)
        if (eligible_bridge_hex(map, x, y))
          map_terrain_set_base(map, x, y, BRIDGE);
}
int map_checkmapfile(BattleMap *map, char *mapname) {
  char *openfile;
  FILE *fp;
  char row[(MAPX * 2) + 3];
  int i = 0;
  int height;
  int width;
  if (strlen(mapname) >= MAP_NAME_SIZE)
    *map_character(mapname, strlen(mapname) + 1, MAP_NAME_SIZE) = 0;
  openfile = map_filename(map, mapname);
  if (!openfile)
    return -1;
  fp = fopen(openfile, "r");
  free(openfile);
  if (!fp) {
    return -1; // Bad map file
  }
  if (!map_read_dimensions(fp, &width, &height) || height < 1 ||
      height > MAPY || width < 1 || width > MAPX) {
    btech_channel_send(map->xcode.context, BTECH_CHANNEL_MAP_ERRORS,
                       "Map #%ld: Invalid height and or/width on %s",
                       map->mynum, mapname);
    if (fclose(fp) != 0)
      return -2;
    return -2; // Bad Height/Width
  }
  // Scan through the mapfile
  for (i = 0; i < height; i++) {
    if (feof(fp) || fgets(row, (2 * MAPX) + 2, fp) == nullptr ||
        strlen(row) < 2U * (size_t)width)
      break;
  }
  if (i != height) {
    btech_channel_send(map->xcode.context, BTECH_CHANNEL_MAP_ERRORS,
                       "Map #%ld: Mapfile possibly corrupt and/or "
                       "height/width flipped. Height != what was read in %s",
                       map->mynum, mapname);
    if (fclose(fp) != 0)
      return -3;
    return -3;
  }
  // Everything is good if we get past the above
  return fclose(fp) == 0 ? 1 : -1;
}
int map_load(BattleMap *map, char *mapname) {
  char *openfile;
  char terr;
  char elev;
  int i1;
  int i2;
  int i3;
  FILE *fp;
  char row[(MAPX * 2) + 3];
  int i;
  int j = 0;
  int height;
  int width;
  if (strlen(mapname) >= MAP_NAME_SIZE)
    *map_character(mapname, strlen(mapname) + 1, MAP_NAME_SIZE) = 0;
  openfile = map_filename(map, mapname);
  if (!openfile)
    return -1;
  fp = fopen(openfile, "r");
  free(openfile);
  if (!fp) {
    return -1; // Bad map file
  }
  del_mapobjs(map); /* Just in case */
  if (map->map) {
    battle_map_grid_destroy(map->map, map->map_height);
    map->map = nullptr;
  }
  if (!map_read_dimensions(fp, &width, &height) || height < 1 ||
      height > MAPY || width < 1 || width > MAPX) {
    btech_channel_send(map->xcode.context, BTECH_CHANNEL_MAP_ERRORS,
                       "Map #%ld: Invalid height and/or width", map->mynum);
    if (fclose(fp) != 0)
      return -1;
    return -1;
  }
  // height is constrained to [1, MAPY] immediately above.
  // NOLINTNEXTLINE(clang-analyzer-optin.taint.TaintedAlloc)
  map->map = battle_map_grid_create(width, height);
  if (map->map == nullptr)
    abort();
  for (i = 0; i < height; i++) {
    if (feof(fp) || fgets(row, (2 * MAPX) + 2, fp) == nullptr ||
        strlen(row) < 2U * (size_t)width) {
      break;
    }
    for (j = 0; j < width; j++) {
      terr = *map_character_const(row, sizeof(row), 2U * (size_t)j);
      elev =
          *map_character_const(row, sizeof(row), (2U * (size_t)j) + 1U) - '0';
      switch (terr) {
      case FIRE:
        map->flags |= MAPFLAG_FIRES;
        break;
      case TFIRE:
      case SMOKE:
      case '.':
        terr = GRASSLAND;
        break;
      case '\'':
        terr = LIGHT_FOREST;
        break;
      }
      if (!strcmp(get_terrain_name_base(terr), "Unknown")) {
        btech_channel_send(map->xcode.context, BTECH_CHANNEL_MAP_ERRORS,
                           "Map #%ld: Invalid terrain at %d,%d: '%c'",
                           map->mynum, j, i, terr);
        terr = GRASSLAND;
      }
      map_hex_set(map, j, i, terr, elev);
    }
  }
  if (i != height) {
    btech_channel_send(map->xcode.context, BTECH_CHANNEL_MAP_ERRORS,
                       "Error: EOF reached prematurely. "
                       "(x%d != %d || y%d != %d)",
                       j, width, i, height);
    if (fclose(fp) != 0)
      return -2;
    return -2;
  }
  map->grav = 100;
  map->temp = 20;
  if (!feof(fp) && fgets(row, sizeof(row), fp) != nullptr) {
    char *separator = strchr(row, ':');
    if (separator != nullptr) {
      char *token_context = nullptr;
      char *grav_text = checked_mutable_string_suffix(separator, 1);
      *separator = '\0';
      grav_text = strtok_r(grav_text, " \t\r\n", &token_context);
      char *temp_text = strtok_r(nullptr, " \t\r\n", &token_context);
      if (grav_text != nullptr && temp_text != nullptr &&
          strtok_r(nullptr, " \t\r\n", &token_context) == nullptr &&
          parse_int_checked(row, &i1) && parse_int_checked(grav_text, &i2) &&
          parse_int_checked(temp_text, &i3)) {
        map->flags = i1;
        map->grav = clamp_int_to_unsigned_char(i2);
        map->temp = clamp_int_to_char(i3);
      }
    }
  }
  map->map_height = clamp_int_to_short(height);
  map->map_width = clamp_int_to_short(width);
  if (!battle_map_disables_bridgification(map))
    make_bridges(map);
  battle_map_name_set(map, mapname);
  if (fclose(fp) != 0)
    return -1;
  return 0;
}
void map_loadmap(DbRef player, void *data, char *buffer) {
  BattleMap *map;
  char *args[1];
  map = (BattleMap *)data;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Loading %s", map_argument(args, 1, 0));
  switch (map_checkmapfile(map, map_argument(args, 1, 0))) {
  case -1:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "#-1 Map not found.");
    return;
  case -2:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "#-1 Map invalid - Bad Height/Width.");
    return;
  case -3:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "#-1 Map invalid - Height not loaded properly");
    return;
  case 1:
    map_load(map, map_argument(args, 1, 0));
    break;
  default:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Unknown error while loading map!");
    return;
  }
  if (player != 1) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Clearing Mechs off Newly Loaded Map");
    map_clearmechs(player, data, "");
    del_mapobjs(map);
  }
}
void map_savemap(DbRef player, void *data, char *buffer) {
  BattleMap *map;
  char *args[1];
  FILE *fp;
  char *openfile;
  int i;
  int j;
  char row[(MAPX * 2) + 1];
  char terrain;
  map = (BattleMap *)data;
  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  char *map_name = map_argument(args, 1, 0);
  if (strlen(map_name) >= MAP_NAME_SIZE)
    *map_character(map_name, strlen(map_name) + 1, MAP_NAME_SIZE) = 0;
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Saving %s", map_name);
  openfile = map_filename(map, map_name);
  if (!openfile) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Unable to open the map file!");
    return;
  }
  fp = fopen(openfile, "w");
  free(openfile);
  if (!fp) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Unable to open the map file!");
    return;
  }
  (void)fprintf(fp, "%d %d\n", map->map_width, map->map_height);
  for (i = 0; i < map->map_height; i++) {
    *map_character(row, sizeof(row), 0) = 0;
    for (j = 0; j < map->map_width; j++) {
      terrain = map_terrain_get(map, j, i);
      switch (terrain) {
      case ' ':
        terrain = '.';
        break;
      case FIRE:
        /* check if we're burnin', if so, alter terrain type */
        if (find_mapobj(&(MapObjectLookupRequest){
                .map = map,
                .position = {.x = j, .y = i},
                .type = TYPE_FIRE,
            })) {
          terrain = TFIRE;
        } else if (!(map->flags & MAPFLAG_FIRES)) {
          map_terrain_set(map, j, i, ' ');
          btech_channel_send(
              map->xcode.context, BTECH_CHANNEL_EVENT_INFO,
              "[lost?] fire event noticed on map #%ld (%s) at %d,%d",
              map->mynum, map->mapname, j, i);
          terrain = '.';
        }
        break;
      case SMOKE:
        terrain = map_real_terrain_get(map, j, i);
        if (terrain == ' ')
          terrain = '.';
        if (terrain == SMOKE) {
          map_terrain_set(map, j, i, ' ');
          btech_channel_send(
              map->xcode.context, BTECH_CHANNEL_EVENT_INFO,
              "[lost?] smoke event noticed on map #%ld (%s) at %d,%d",
              map->mynum, map->mapname, j, i);
          terrain = '.';
        }
        break;
      }
      *map_character(row, sizeof(row), 2U * (size_t)j) = terrain;
      *map_character(row, sizeof(row), (2U * (size_t)j) + 1U) =
          map_elevation_get(map, j, i) + '0';
    }
    *map_character(row, sizeof(row), 2U * (size_t)j) = 0;
    (void)fprintf(fp, "%s\n", row);
  }
  i = map->flags & ~MAPFLAG_MAPO;
  if (i)
    (void)fprintf(fp, "%d: %d %d\n", i, map->grav, map->temp);
  if (fclose(fp) != 0) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Unable to finish saving the map file.");
    return;
  }
  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "Saving complete!");
}
void map_setmapsize(DbRef player, void *data, char *buffer) {
  BattleMap *oldmap;
  unsigned char **map;
  int x;
  int y;
  int i;
  int j;
  int failed = 0;
  int x1;
  int y1;
  char *args[4];
  oldmap = (BattleMap *)data;
  if (first_mapobj(oldmap, TYPE_BITS)) {
    mecha_notify(btech_context_evaluation(oldmap->xcode.context), player,
                 "Invalid map for size change, sorry.");
    return;
  }
  if (mech_parseattributes(buffer, args, 4) != 2) {
    mecha_notify(btech_context_evaluation(oldmap->xcode.context), player,
                 "Invalid number of arguments (X/Y expected)");
    return;
  }
  if (!parse_int_checked(map_argument(args, 4, 0), &x) ||
      !parse_int_checked(map_argument(args, 4, 1), &y) || x < 0 || x > MAPX ||
      y < 0 || y > MAPY) {
    mecha_notify(btech_context_evaluation(oldmap->xcode.context), player,
                 "X,Y out of range!");
    return;
  }
  /* allocate new map space */
  map = battle_map_grid_create(x, y);
  if (map == nullptr)
    failed = 1;
  if (failed) {
    btech_channel_send(oldmap->xcode.context, BTECH_CHANNEL_MAP_ERRORS,
                       "Memory allocation failed in setmapsize!");
  } else {
    /* Initialize the hexes in the new map to blank */
    for (i = 0; i < y; i++)
      for (j = 0; j < x; j++)
        map_hex_buffer_set(&oldmap->xcode.context->map_coding, map, x, y, j, i,
                           ' ', 0);
    /* Copy old map into new map */
    x1 = (oldmap->map_width < x) ? oldmap->map_width : x;
    y1 = (oldmap->map_height < y) ? oldmap->map_height : y;
    for (i = 0; i < y1; i++)
      for (j = 0; j < x1; j++)
        map_hex_buffer_set(&oldmap->xcode.context->map_coding, map, x, y, j, i,
                           map_terrain_get(oldmap, j, i),
                           map_elevation_get(oldmap, j, i));
    /* Now free the old map */
    battle_map_grid_destroy(oldmap->map, oldmap->map_height);
    del_mapobjs(oldmap);
    /* set new map size and pointer to new map space */
    oldmap->map_height = clamp_int_to_short(y);
    oldmap->map_width = clamp_int_to_short(x);
    oldmap->map = map;
    mecha_notify(btech_context_evaluation(oldmap->xcode.context), player,
                 "Size set.");
  }
}
void map_clearmechs(DbRef player, void *data,
                    const char *buffer [[maybe_unused]]) {
  BattleMap *map;
  map = (BattleMap *)data;
  if (map != nullptr)
    map_shutdown_units(&(MapShutdownRequest){
        .context = map->xcode.context, .actor = player, .map = map->mynum});
}
void map_update(DbRef obj, void *data) {
  BattleMap *map = ((BattleMap *)data);
  Mech *mech;
  char *tmps;
  char *changemsg = alloc_lbuf("map_update.change");
  char *attribute_buffer = alloc_lbuf("map_update.attribute");
  int ma = 30;
  int ml = 2;
  int wind = 0;
  int wspeed = 0;
  int cloudbase = 200;
  int oldl;
  int oldv;
  int i;
  /* Changed from % 25 to % 60. %60 never hit when the event tick came here
     and was odd. %25 should hit when it is odd or even. */
  if (!(map->xcode.context->events->tick % 25)) {
    oldl = (unsigned char)map->maplight;
    oldv = (unsigned char)map->mapvis;
    bool valid_map_visibility =
        ((tmps = btech_attribute_read(map->xcode.context->database, obj,
                                      A_MAPVIS, attribute_buffer)) != nullptr &&
         map_parse_visibility_attribute(tmps, &ma, &ml, &wind, &wspeed,
                                        &cloudbase, changemsg, LBUF_SIZE)) != 0;
    if (!valid_map_visibility) {
      ma = 30;
      ml = 2;
      wind = 0;
      wspeed = 0;
      cloudbase = 200;
    }
    map->winddir = clamp_int_to_short(wind);
    map->windspeed = clamp_int_to_short(wspeed);
    map->mapvis = clamp_int_to_char(bounded(0, ma, 60));
    map->maxvis = clamp_int_to_short(bounded(24, ma * 3, 60));
    map->maplight = clamp_int_to_char(bounded(0, ml, 2));
    map->cloudbase = clamp_int_to_short(cloudbase);
    if (ml != oldl || ma != oldv) {
      for (i = 0; i < battle_map_unit_count(map); i++) {
        const DbRef MECH_DBREF = battle_map_unit_dbref(map, i);
        if (MECH_DBREF < 0)
          continue;
        mech = btech_context_get_mech(map->xcode.context, MECH_DBREF);
        if (!mech)
          continue;
        if (ml != oldl)
          sensor_light_availability_check(mech);
        if (strlen(changemsg) > 5)
          mech_notify(mech, MECHALL, changemsg);
      }
    }
  }
  mech_sensor_map_los_update(obj, map);
  free_buf(attribute_buffer);
  free_buf(changemsg);
  /* Fire/Smoke are event-driven -> nothing related to them done here */
}
void initialize_map_empty(BattleMap *new, DbRef key) {
  int i;
  int j;
  new->mynum = key;
  new->map_width = DEFAULT_MAP_WIDTH;
  new->map_height = DEFAULT_MAP_HEIGHT;
  new->regen_factor = 1; /* Default the building regen to 1 */
  new->map = battle_map_grid_create(new->map_width, new->map_height);
  if (new->map == nullptr)
    abort();
  for (i = 0; i < new->map_height; i++)
    for (j = 0; j < new->map_width; j++)
      map_hex_set(new, j, i, BATTLE_TERRAIN_GRASSLAND, 0);
}
/* Mem alloc/free routines */
void newfreemap(DbRef key, void **data,
                BtechSpecialLifecycleOperation selector) {
  BattleMap *new = *data;
  switch (selector) {
  case SPECIAL_ALLOC:
    initialize_map_empty(new, key);
    /* allocate default map space */
    memset((void *)new->map_object, 0, sizeof(new->map_object));
    (void)snprintf(new->mapname, MAP_NAME_SIZE, "%s", "Default Map");
    break;
  case SPECIAL_FREE:
    /* Seriously. We weren't clearing the map of mechas. Bad bad accounting!!!
     */
    map_clearmechs(GOD, new, "");
    del_mapobjs(new);
    if (new->map) {
      battle_map_grid_destroy(new->map, new->map_height);
    }
    battle_map_dynamic_destroy(new);
    break;
  }
}
