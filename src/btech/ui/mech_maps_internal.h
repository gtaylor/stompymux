#pragma once
#include <assert.h>
#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "aero_move_api.h"
#include "autopilot.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "ds_bay_api.h"
#include "eject_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_los.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_maps_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_restrict_api.h"
#include "mech_utils_api.h"
#include "mine.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/world/access.h"
#include "mux/world/move.h"
#include "registry_api.h"
enum {
  SWATER_IDX,
  DWATER_IDX,
  BUILDING_IDX,
  ROAD_IDX,
  ROUGH_IDX,
  MOUNTAIN_IDX,
  FIRE_IDX,
  ICE_IDX,
  WALL_IDX,
  SNOW_IDX,
  SMOKE_IDX,
  LWOOD_IDX,
  HWOOD_IDX,
  UNKNOWN_IDX,
  CLIFF_IDX,
  SELF_IDX,
  FRIEND_IDX,
  ENEMY_IDX,
  DS_IDX,
  GOODLZ_IDX,
  BADLZ_IDX,
  NUM_COLOR_IDX
};

/* Default colour string is "BbWnYyRWWWXGgbRHYRn" */
/* internal rep has H instead of h and \0 instead of n */

#define DEFAULT_COLOR_STRING "BbWXYyRWWWXGgbRhYRnGR"
#define DEFAULT_COLOR_SCHEME "BbWXYyRWWWXGgbRHYR\0GR"

typedef struct MapColorScheme {
  char values[NUM_COLOR_IDX + 1];
} MapColorScheme;

typedef struct MapCellText {
  char text[32];
} MapCellText;

struct MapText {
  char *buffer;
  char **lines;
  size_t buffer_capacity;
  size_t line_capacity;
};

void map_color_scheme_load(MapColorScheme *colors, BtechContext *context,
                           DbRef player);
char map_terrain_color_char(const MapColorScheme *colors, char terrain,
                            int elevation);
const char *map_color_markup(char color);
bool style_tac_map(MapText *text, const MapColorScheme *colors,
                   const char *sketch, int dispcols, int disprows);
int parse_tacargs(DbRef player, Mech *mech, char **args, int argc, int maxrange,
                  short *x, short *y);
