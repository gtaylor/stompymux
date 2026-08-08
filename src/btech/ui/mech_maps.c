#include "btech/context.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_map_render_internal.h"
#include "mech_maps_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

#include "mux/support/formatting.h"
#include <stdlib.h>
#include <string.h>

void mech_findcenter(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  float fx, fy;
  int x, y;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  x = mech_position_x(mech);
  y = mech_position_y(mech);
  MapCoordToRealCoord(x, y, &fx, &fy);
  notify_printf(evaluation, player,
                "Current hex: (%d,%d,%d)\tRange to center: %.2f\t"
                "Bearing to center: %d",
                x, y, mech_position_z(mech),
                FindHexRange(fx, fy, mech_position_real_x(mech),
                             mech_position_real_y(mech)),
                FindBearing(mech_position_real_x(mech),
                            mech_position_real_y(mech), fx, fy));
}

int parse_tacargs(DbRef player, Mech *mech, char **args, int argc, int maxrange,
                  short *x, short *y) {
  int bearing;
  float range, fx, fy;
  Mech *tempMech;
  BattleMap *map;

  switch (argc) {
  case 2:
    if (!parse_int_checked(args[0], &bearing) ||
        !parse_float_checked(args[1], &range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid bearing or range.");
      return 0;
    }
    if (!mech_is_observer(mech) && abs((int)range) > maxrange) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of sensor range!");
      return 0;
    }
    FindXY(mech_position_real_x(mech), mech_position_real_y(mech), bearing,
           range, &fx, &fy);
    RealCoordToMapCoord(x, y, fx, fy);
    return 1;
  case 1:
    map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    tempMech =
        btech_context_get_mech(mech_context(mech), FindMechOnMap(map, args[0]));
    if (!tempMech) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "No such target.");
      return 0;
    }
    range = mech_range_to(mech, tempMech);
    if (!mech_los_check(mech, tempMech, mech_position_x(tempMech),
                        mech_position_y(tempMech), range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "No such target.");
      return 0;
    }
    if (abs((int)range) > maxrange) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Target is out of scanner range.");
      return 0;
    }
    *x = mech_position_x(tempMech);
    *y = mech_position_y(tempMech);
    return 1;
  case 0:
    *x = mech_position_x(mech);
    *y = mech_position_y(mech);
    return 1;
  default:
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of parameters!");
    return 0;
  }
}

const char *GetTerrainName_base(int t) {
  switch (t) {
  case GRASSLAND:
  case '_':
    return "Grassland";
  case HEAVY_FOREST:
    return "Heavy Forest";
  case LIGHT_FOREST:
    return "Light Forest";
  case ICE:
    return "Ice";
  case BRIDGE:
    return "Bridge";
  case HIGHWATER:
  case WATER:
    return "Water";
  case ROUGH:
    return "Rough";
  case MOUNTAINS:
    return "Mountains";
  case ROAD:
    return "Road";
  case BUILDING:
    return "Building";
  case FIRE:
    return "Fire";
  case SMOKE:
    return "Smoke";
  case WALL:
    return "Wall";
  case DESERT:
    return "Desert";
  }
  return "Unknown";
}

const char *GetTerrainName(BattleMap *map, int x, int y) {
  return GetTerrainName_base(map_terrain_get(map, x, y));
}

/* Player-customizable colors */

void map_color_scheme_load(MapColorScheme *colors) {
  memcpy(colors->values, DEFAULT_COLOR_SCHEME, NUM_COLOR_IDX);
}

void mech_navigate(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char mybuff[NAVIGATE_LINES][MBUF_SIZE];
  BattleMap *mech_map;
  char *const *maptext;
  MapText *map_text;
  char *args[3];
  int i, dolos, argc;
  short x, y;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));

  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  dolos = battle_map_is_dark(mech_map) ||
          (mech_class(mech) == CLASS_MW &&
           mech_context(mech)->configuration->btech_mw_losmap);

  if (mech_map->map_width <= 0 || mech_map->map_height <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Nothing to see on this map, move along.");
    return;
  }

  argc = mech_parseattributes(buffer, args, 3);
  if (!parse_tacargs(player, mech, args, argc, mech_tactical_range(mech), &x,
                     &y))
    return;

  map_text = map_text_create(player, mech, mech_map, x, y, 5, 5, 4, dolos);
  if (map_text == nullptr) {
    mecha_notify(evaluation, player, "Unable to render the tactical map.");
    return;
  }
  maptext = map_text_lines(map_text);

  snprintf(mybuff[0], MBUF_SIZE,
           "              0                                          %.150s",
           maptext[0]);
  snprintf(mybuff[1], MBUF_SIZE,
           "         ___________                                     %.150s",
           maptext[1]);
  snprintf(mybuff[2], MBUF_SIZE,
           "        /           \\          Location:%4d,%4d, %3d   %.150s",
           mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
           maptext[2]);
  snprintf(
      mybuff[3], MBUF_SIZE,
      "  300  /             \\  60     Terrain: %14s   %.150s",
      GetTerrainName(mech_map, mech_position_x(mech), mech_position_y(mech)),
      maptext[3]);
  snprintf(mybuff[4], MBUF_SIZE,
           "      /               \\                                  %.150s",
           maptext[4]);
  snprintf(mybuff[5], MBUF_SIZE,
           "     /                 \\                                 %.150s",
           maptext[5]);
  snprintf(mybuff[6], MBUF_SIZE,
           "270 (                   )  90  Speed:           %6.1f   %.150s",
           mech_current_speed(mech), maptext[6]);
  snprintf(mybuff[7], MBUF_SIZE,
           "     \\                 /       Vertical Speed:  %6.1f   %.150s",
           mech_vertical_speed(mech), maptext[7]);
  snprintf(mybuff[8], MBUF_SIZE,
           "      \\               /        Heading:           %4d   %.150s",
           mech_heading_degrees(mech), maptext[8]);
  snprintf(mybuff[9], MBUF_SIZE,
           "  240  \\             /  120                              %.150s",
           maptext[9]);
  snprintf(mybuff[10], MBUF_SIZE,
           "        \\___________/                                    %.150s",
           maptext[10]);
  snprintf(mybuff[11], MBUF_SIZE, "                      ");
  snprintf(mybuff[12], MBUF_SIZE, "             180");
  map_text_destroy(map_text);

  navigate_sketch_mechs(mech, mech_map, x, y, mybuff);
  for (i = 0; i < NAVIGATE_LINES; i++)
    mecha_notify(evaluation, player, mybuff[i]);
}

/* INDENT OFF */

/*
   0
   ___________                                     /``\][/""\][/""\
   /           \          HEX Location: 254, 122    \`1/``\""/``\""/
   300  /             \  60     Terrain: Light Forest     /``\``/""\`3/""\
   /               \        Elevation:  0             \`2/``\"1/``\""/
   /                 \                                 /""\``|**\`3/""\
   270 (                   )  90  Speed: 0.0                \"4/``\"4/``\""/
   \                 /       Vertical Speed: 0.0       /""\`3/""\`3/""\
   \               /        Heading: 0                \"4/``\"4/``\""/
   240  \             /  120                              /""\`3/""\`3/""\
   \____*______/                                    \"4/][\"4/][\"4/
   180
   */

/* INDENT ON */
