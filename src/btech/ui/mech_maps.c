#include "btech/context.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
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
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

#include "section_types.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *navigate_line(char lines[NAVIGATE_LINES][MBUF_SIZE], int index) {
  return checked_storage_at(lines, NAVIGATE_LINES, sizeof(*lines),
                            (size_t)index);
}

typedef struct NavigateCanvas {
  char (*lines)[MBUF_SIZE];
} NavigateCanvas;

static void navigate_plot(const NavigatePlotCall *call) {
  NavigateCanvas *canvas = call->context;
  char *line = navigate_line(canvas->lines, call->row);
  char *cell =
      checked_storage_at(line, MBUF_SIZE, sizeof(*line), (size_t)call->column);
  *cell = call->marker;
}

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
  notify_printf(
      evaluation, player,
      "Current hex: (%d,%d,%d)\tRange to center: %.2f\t"
      "Bearing to center: %d",
      x, y, mech_position_z(mech),
      (double)map_real_range(&(MapRealSegment){
          .start = {.x = fx, .y = fy},
          .end = {.x = mech_position_real_x(mech),
                  .y = mech_position_real_y(mech)},
      }),
      map_bearing(&(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                              .y = mech_position_real_y(mech)},
                                    .end = {.x = fx, .y = fy}}));
}

static char *tactical_argument(char *const *args, size_t argument_capacity,
                               size_t index) {
  return *(char *const *)checked_storage_at_const(
      (const void *)args, argument_capacity, sizeof(*args), index);
}

TacticalArgumentParseResult
tactical_arguments_parse(const TacticalArgumentParseRequest *request) {
  const DbRef player = request->player;
  Mech *mech = request->mech;
  char *const *args = request->arguments;
  const size_t argument_capacity = request->argument_capacity;
  const size_t first_argument = request->first_argument;
  const int argc = request->argument_count;
  const int maxrange = request->maximum_range;
  TacticalArgumentParseResult result = {0};
  int bearing;
  float range, fx, fy;
  Mech *tempMech;
  BattleMap *map;

  switch (argc) {
  case 2:
    if (!parse_int_checked(
            tactical_argument(args, argument_capacity, first_argument),
            &bearing) ||
        !parse_float_checked(
            tactical_argument(args, argument_capacity, first_argument + 1),
            &range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid bearing or range.");
      return result;
    }
    if (!mech_is_observer(mech) && abs((int)range) > maxrange) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of sensor range!");
      return result;
    }
    MapRealPosition projected = map_project_position(
        &(MapProjection){.origin = {.x = mech_position_real_x(mech),
                                    .y = mech_position_real_y(mech)},
                         .bearing = bearing,
                         .range = range});
    fx = projected.x;
    fy = projected.y;
    short x;
    short y;
    RealCoordToMapCoord(&x, &y, fx, fy);
    result.valid = true;
    result.position = (MapHexPosition){.x = x, .y = y};
    return result;
  case 1:
    map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    tempMech = btech_context_get_mech(
        mech_context(mech),
        FindMechOnMap(
            map, tactical_argument(args, argument_capacity, first_argument)));
    if (!tempMech) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "No such target.");
      return result;
    }
    range = mech_range_to(mech, tempMech);
    if (!mech_los_check(mech, tempMech, mech_position_x(tempMech),
                        mech_position_y(tempMech), range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "No such target.");
      return result;
    }
    if (abs((int)range) > maxrange) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Target is out of scanner range.");
      return result;
    }
    result.valid = true;
    result.position = (MapHexPosition){.x = mech_position_x(tempMech),
                                       .y = mech_position_y(tempMech)};
    return result;
  case 0:
    result.valid = true;
    result.position = (MapHexPosition){.x = mech_position_x(mech),
                                       .y = mech_position_y(mech)};
    return result;
  default:
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of parameters!");
    return result;
  }
}

const char *GetTerrainName_base(int t) {
  switch (t) {
  case GRASSLAND:
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
  NavigateCanvas canvas = {.lines = mybuff};
  BattleMap *mech_map;
  MapText *map_text;
  char *args[3];
  int i, dolos, argc;
  int x, y;

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
  const TacticalArgumentParseResult parsed =
      tactical_arguments_parse(&(TacticalArgumentParseRequest){
          .player = player,
          .mech = mech,
          .arguments = args,
          .argument_capacity = 3,
          .first_argument = 0,
          .argument_count = argc,
          .maximum_range = mech_tactical_range(mech),
      });
  if (!parsed.valid)
    return;
  x = parsed.position.x;
  y = parsed.position.y;

  MapTextRequest request = {
      .player = player,
      .mech = mech,
      .map = mech_map,
      .center_x = x,
      .center_y = y,
      .width = 5,
      .height = 5,
      .labels = 4,
      .calculate_los = dolos,
  };
  map_text = map_text_create(&request);
  if (map_text == nullptr) {
    mecha_notify(evaluation, player, "Unable to render the tactical map.");
    return;
  }
  (void)snprintf(
      navigate_line(mybuff, 0), MBUF_SIZE,
      "              0                                          %.150s",
      map_text_line(map_text, 0));
  (void)snprintf(
      navigate_line(mybuff, 1), MBUF_SIZE,
      "         ___________                                     %.150s",
      map_text_line(map_text, 1));
  (void)snprintf(
      navigate_line(mybuff, 2), MBUF_SIZE,
      "        /           \\          Location:%4d,%4d, %3d   %.150s",
      mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
      map_text_line(map_text, 2));
  (void)snprintf(
      navigate_line(mybuff, 3), MBUF_SIZE,
      "  300  /             \\  60     Terrain: %14s   %.150s",
      GetTerrainName(mech_map, mech_position_x(mech), mech_position_y(mech)),
      map_text_line(map_text, 3));
  (void)snprintf(
      navigate_line(mybuff, 4), MBUF_SIZE,
      "      /               \\                                  %.150s",
      map_text_line(map_text, 4));
  (void)snprintf(
      navigate_line(mybuff, 5), MBUF_SIZE,
      "     /                 \\                                 %.150s",
      map_text_line(map_text, 5));
  (void)snprintf(
      navigate_line(mybuff, 6), MBUF_SIZE,
      "270 (                   )  90  Speed:           %6.1f   %.150s",
      (double)mech_current_speed(mech), map_text_line(map_text, 6));
  (void)snprintf(
      navigate_line(mybuff, 7), MBUF_SIZE,
      "     \\                 /       Vertical Speed:  %6.1f   %.150s",
      (double)mech_vertical_speed(mech), map_text_line(map_text, 7));
  (void)snprintf(
      navigate_line(mybuff, 8), MBUF_SIZE,
      "      \\               /        Heading:           %4d   %.150s",
      mech_heading_degrees(mech), map_text_line(map_text, 8));
  (void)snprintf(
      navigate_line(mybuff, 9), MBUF_SIZE,
      "  240  \\             /  120                              %.150s",
      map_text_line(map_text, 9));
  (void)snprintf(
      navigate_line(mybuff, 10), MBUF_SIZE,
      "        \\___________/                                    %.150s",
      map_text_line(map_text, 10));
  (void)snprintf(navigate_line(mybuff, 11), MBUF_SIZE,
                 "                      ");
  (void)snprintf(navigate_line(mybuff, 12), MBUF_SIZE, "             180");
  map_text_destroy(map_text);

  navigate_sketch_mechs(&(NavigateSketchRequest){
      .mech = mech,
      .map = mech_map,
      .center = {.x = x, .y = y},
      .plot = navigate_plot,
      .context = &canvas,
  });
  for (i = 0; i < NAVIGATE_LINES; i++)
    mecha_notify(evaluation, player, navigate_line(mybuff, i));
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
