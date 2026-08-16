#include "btech/context.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "context_internal.h"
#include "map.h"
#include "map_api.h"
#include "map_conditions_api.h"
#include "map_name_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_maps_api.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

static unsigned char grid_cell;
static unsigned char *grid_rows[] = {&grid_cell};
static char loaded_terrain;
static char loaded_elevation;

int bounded(int low, int value, int high);

int bounded(int low, int value, int high) {
  return value < low ? low : value > high ? high : value;
}

void del_mapobjs(BattleMap *map [[maybe_unused]]) {}

unsigned char **battle_map_grid_create(int width, int height) {
  return width == 1 && height == 1 ? grid_rows : nullptr;
}

void battle_map_grid_destroy(unsigned char **grid [[maybe_unused]],
                             int height [[maybe_unused]]) {}

void map_hex_set(BattleMap *map [[maybe_unused]], int x [[maybe_unused]],
                 int y [[maybe_unused]], char terrain, char elevation) {
  loaded_terrain = terrain;
  loaded_elevation = elevation;
}

char map_terrain_get(const BattleMap *map [[maybe_unused]],
                     int x [[maybe_unused]], int y [[maybe_unused]]) {
  return GRASSLAND;
}

void map_terrain_set_base(BattleMap *map [[maybe_unused]],
                          int x [[maybe_unused]], int y [[maybe_unused]],
                          char terrain [[maybe_unused]]) {}

bool battle_map_disables_bridgification(const BattleMap *map [[maybe_unused]]) {
  return true;
}

const char *get_terrain_name_base(int terrain [[maybe_unused]]) {
  return "Grassland";
}

bool map_read_dimensions(FILE *file, int *width, int *height) {
  char line[64];
  char *token_context = nullptr;
  if (fgets(line, sizeof(line), file) == nullptr)
    return false;
  char *width_text = strtok_r(line, " \t\r\n", &token_context);
  char *height_text = strtok_r(nullptr, " \t\r\n", &token_context);
  return width_text != nullptr && height_text != nullptr &&
         strtok_r(nullptr, " \t\r\n", &token_context) == nullptr &&
         parse_int_checked(width_text, width) &&
         parse_int_checked(height_text, height);
}

void btech_channel_send(BtechContext *context [[maybe_unused]],
                        BtechChannel channel [[maybe_unused]],
                        const char *format [[maybe_unused]], ...) {
  va_list arguments;
  va_start(arguments, format);
  va_end(arguments);
}

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;

  char path[256];
  char *const *argument = (char *const *)checked_storage_at_const(
      argv, (size_t)argc, sizeof(*argv), 1);
  const char *fixture_path = *argument;
  (void)string_copy_bounded(path, sizeof(path), fixture_path);
  char *separator = strrchr(path, '/');
  if (separator == nullptr)
    return 3;
  const size_t separator_offset = (size_t)(separator - path);
  char *map_name = checked_mutable_string_suffix(path, separator_offset + 1);
  *separator = '\0';

  ServerConfiguration configuration = {};
  (void)snprintf(configuration.database.map_db,
                 sizeof(configuration.database.map_db), "%s", path);
  BtechContext context = {.configuration = &configuration};
  BattleMap map = {.xcode = {.context = &context}};

  const int RESULT = map_load(&map, map_name);
  if (RESULT != 0 || map.grav != 88 || map.temp != 19 || map.flags != 42 ||
      map.map_width != 1 || map.map_height != 1 ||
      loaded_terrain != GRASSLAND || loaded_elevation != 0) {
    (void)fprintf(stderr,
                  "result=%d grav=%u temp=%d flags=%d size=%dx%d terrain=%d "
                  "elevation=%d\n",
                  RESULT, map.grav, map.temp, map.flags, map.map_width,
                  map.map_height, loaded_terrain, loaded_elevation);
    return 1;
  }
  return 0;
}
