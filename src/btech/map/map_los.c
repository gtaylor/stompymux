
/* Implements line-of-sight calculations on maps. */

#include "map_los.h"
#include "equipment_types.h"
#include "map_los_api.h"

#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "btech_channel.h"
#include "btech_event.h" // IWYU pragma: keep
#include "command_handlers_api.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_los_types.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_lostracer_api.h"
#include "mech_position_api.h"
#include "mech_sensor.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

static unsigned short *battle_map_los_cell(BattleMap *map, int observer,
                                           int target) {
  if (observer < 0 || target < 0)
    abort();
  unsigned short **row_slot = (unsigned short **)checked_storage_at(
      (void *)map->lo_sinfo, (size_t)map->dynamic_size, sizeof(*map->lo_sinfo),
      (size_t)observer);
  return checked_storage_at(*row_slot, (size_t)map->dynamic_size,
                            sizeof(**row_slot), (size_t)target);
}

static const unsigned short *
battle_map_los_cell_const(const BattleMap *map, int observer, int target) {
  if (observer < 0 || target < 0)
    abort();
  unsigned short *const *row_slot =
      (unsigned short *const *)checked_storage_at_const(
          (const void *)map->lo_sinfo, (size_t)map->dynamic_size,
          sizeof(*map->lo_sinfo), (size_t)observer);
  return checked_storage_at_const(*row_slot, (size_t)map->dynamic_size,
                                  sizeof(**row_slot), (size_t)target);
}

static unsigned char *los_map_cell(HexLosMap *los_map, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(los_map->map,
                            (size_t)MAPLOS_MAXX * (size_t)MAPLOS_MAXY,
                            sizeof(*los_map->map), (size_t)index);
}

static const unsigned char *los_map_cell_const(const HexLosMap *los_map,
                                               int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(los_map->map,
                                  (size_t)MAPLOS_MAXX * (size_t)MAPLOS_MAXY,
                                  sizeof(*los_map->map), (size_t)index);
}

static const LosTracePoint *los_trace_point(const LosTrace *trace, int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(trace->points, LOS_TRACE_CAPACITY,
                                  sizeof(*trace->points), (size_t)index);
}

static int los_map_index_x(const HexLosMap *los_map, int index) {
  return index % los_map->xsize + los_map->startx;
}

static int los_map_index_y(const HexLosMap *los_map, int index) {
  return index / los_map->xsize + los_map->starty;
}

bool battle_map_unit_is_seen(const BattleMap *map, const Mech *observer,
                             const Mech *target) {
  return *battle_map_los_cell_const(map, mech_map_slot(observer),
                                    mech_map_slot(target)) &
         BATTLE_MAP_LOS_SEEN;
}

bool battle_map_unit_los_is_blocked(const BattleMap *map, const Mech *observer,
                                    const Mech *target) {
  return *battle_map_los_cell_const(map, mech_map_slot(observer),
                                    mech_map_slot(target)) &
         BATTLE_MAP_LOS_BLOCKED;
}

int battle_map_unit_los_wood_count(const BattleMap *map, const Mech *observer,
                                   const Mech *target) {
  const int FLAGS = *battle_map_los_cell_const(map, mech_map_slot(observer),
                                               mech_map_slot(target));
  return battle_map_los_wood_count(FLAGS);
}

int battle_map_unit_los_water_count(const BattleMap *map, const Mech *observer,
                                    const Mech *target) {
  const int FLAGS = *battle_map_los_cell_const(map, mech_map_slot(observer),
                                               mech_map_slot(target));
  return battle_map_los_water_count(FLAGS);
}

unsigned short battle_map_los_flags(const BattleMap *map, int observer_index,
                                    int target_index) {
  return *battle_map_los_cell_const(map, observer_index, target_index);
}

void battle_map_los_flags_set(BattleMap *map, int observer_index,
                              int target_index, unsigned short flags) {
  *battle_map_los_cell(map, observer_index, target_index) = flags;
}

void battle_map_los_observer_clear(BattleMap *map, int observer_index) {
  for (int target_index = 0; target_index < map->first_free; target_index++)
    *battle_map_los_cell(map, observer_index, target_index) = 0;
}

int los_map_hex_index(const HexLosMap *map_info, int x, int y) {
  if (x < map_info->startx || x > map_info->startx + map_info->xsize ||
      y < map_info->starty || y > map_info->starty + map_info->ysize) {
    btech_channel_send(
        map_info->context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("LOSMap request from out of bounds hex: %d,%d", x, y));
    return 0;
  }
  return ((y - map_info->starty) * map_info->xsize) + (x - map_info->startx);
}

unsigned char los_map_flag(const HexLosMap *los_map, int x, int y) {
  return *los_map_cell_const(los_map, los_map_hex_index(los_map, x, y));
}

static float mech_los_height(Mech *mech) {
  switch (mech_class(mech)) {
  case CLASS_MECH:
    return 0.2F + (float)!mech_is_fallen(mech);
  case CLASS_SPHEROID_DS:
    return 4.2F;
  case CLASS_DS:
    return 2.2F;
  case CLASS_MW:
  case CLASS_VEH_NAVAL:
    return 0.01F;
  case CLASS_VEH_GROUND:
  case CLASS_VTOL:
  case CLASS_AERO:
  case CLASS_BSUIT:
    break;
  default:
    break;
  }
  return 0.2F;
}

static void set_hexlosinfo(HexLosMap *los_map, int x, int y, int flag) {
  if (x < los_map->startx || x >= los_map->startx + los_map->xsize ||
      y < los_map->starty || y >= los_map->starty + los_map->ysize) {
    return;
  }
  *los_map_cell(los_map, los_map_hex_index(los_map, x, y)) |=
      flag | MAPLOSHEX_SEEN;
}

static int hexlit(HexLosMap *los_map, int x, int y) {
  if (x < los_map->startx || x >= los_map->startx + los_map->xsize ||
      y < los_map->starty || y >= los_map->starty + los_map->ysize) {
    return 0;
  }

  return *los_map_cell(los_map, los_map_hex_index(los_map, x, y)) &
         MAPLOSHEX_LIT;
}

static void set_sliteinfo(HexLosMap *los_map, int x, int y, int flag) {
  if (x < los_map->startx || x >= los_map->startx + los_map->xsize ||
      y < los_map->starty || y >= los_map->starty + los_map->ysize) {
    return;
  }
  los_map->flags |= MAPLOS_FLAG_SLITE;
  *los_map_cell(los_map, los_map_hex_index(los_map, x, y)) |= flag;
}

/* To efficiently set all hexes NOLOS if neither sensor supports seeing
 * terrain, and (in the future) to set all hexes LOSALL if either sensor
 * sees all terrain (e.g. 'sattelite downlink' sensor)
 */

static void set_hexlosall(HexLosMap *los_map, int flag) {
  memset(los_map->map, flag | MAPLOSHEX_SEEN,
         (size_t)los_map->xsize * (size_t)los_map->ysize);
}

/* The following functions are, effectively, STUBS. They should be
   replaced with functions in the sensor struct, instead of their
   functionality being copied all over the tree. */

typedef struct SensorObstructionRequest {
  Mech *mech;
  BattleMap *map;
  int count;
  int sensor;
  int los_flag;
} SensorObstructionRequest;

static int
mech_los_sees_through_obstruction(const SensorObstructionRequest *request) {
  int sn = mech_sensor_index(request->mech, request->sensor);
  int fake_losflag = request->count * request->los_flag;
  SensorContactRequest contact = {
      .observer = request->mech,
      .map = request->map,
      .range = 1,
      .flags = fake_losflag,
  };
  int res = mech_sensor_definition(sn)->can_see(&contact);

  return res;
}

static int mech_los_sees_over_mountain(Mech *mech, BattleMap *map, int sensor) {
  int sn = mech_sensor_index(mech, sensor);
  int fake_losflag = BATTLE_MAP_LOS_MOUNTAIN;

  SensorContactRequest request = {
      .observer = mech,
      .map = map,
      .range = 1,
      .flags = fake_losflag,
  };
  return mech_sensor_definition(sn)->can_see(&request);
}

typedef struct SensorRangeRequest {
  HexLosMap *los_map;
  Mech *mech;
  BattleMap *map;
  MapHexPosition position;
  int elevation;
  int sensor;
} SensorRangeRequest;

static int mech_los_sees_range(const SensorRangeRequest *request) {
  HexLosMap *los_map = request->los_map;
  Mech *mech = request->mech;
  BattleMap *map = request->map;
  int sn = mech_sensor_index(mech, request->sensor);
  float fx;
  float fy;
  float range;
  const SensorDefinition *sensor_definition = mech_sensor_definition(sn);
  float maxvis = (float)sensor_definition->maximum_visibility;

  map_coord_to_real_coord(request->position.x, request->position.y, &fx, &fy);
  range = map_spatial_range(&(MapSpatialSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech),
                .z = mech_position_real_z(mech)},
      .end = {.x = fx, .y = fy, .z = ZSCALE * (float)request->elevation},
  });

  /* XXX HACK: code duplication. this should be replaced with sensor
   * functions
   */

  if (sn < 2) /* V or L sensors */
    maxvis = map->mapvis;
  if (sn == 1 && map->maplight == 0) /* L sensors in darkness */
    maxvis *= 2;

  if (!sensor_definition->full_vision) {
    int arc = in_weapon_arc(mech, fx, fy);

    if (!(arc & (FORWARDARC | TURRETARC))) {
      if (mech_sensor_index(mech, 0) == mech_sensor_index(mech, 1))
        maxvis = (maxvis * 200 / 300);
      else
        maxvis = 0;
    }
  }

  if (sn == 0 && maxvis > 0.0F && range >= maxvis &&
      (los_map->flags & MAPLOS_FLAG_SLITE))
    return -1;

  return range < maxvis;
}

typedef struct SearchlightReachRequest {
  Mech *mech;
  MapHexPosition position;
  int elevation;
} SearchlightReachRequest;

static int mech_searchlight_reaches(const SearchlightReachRequest *request) {
  Mech *mech = request->mech;
  float fx;
  float fy;
  float range;
  int arc;
  const float MAXVIS = 60.0F;

  map_coord_to_real_coord(request->position.x, request->position.y, &fx, &fy);
  arc = in_weapon_arc(mech, fx, fy);
  if (!(arc & (FORWARDARC | TURRETARC))) {
    return 0;
  }

  range = map_spatial_range(&(MapSpatialSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech),
                .z = mech_position_real_z(mech)},
      .end = {.x = fx, .y = fy, .z = ZSCALE * (float)request->elevation},
  });
  return range < MAXVIS;
}

static int mech_sensor_sees_terrain(Mech *mech, int sn) {
  return mech_sensor_index(mech, sn) < 4;
}

/* General idea: stateful LOS checking.

 * To minimize the number of lostracing we do, we calculate the losmap by
 * tracing los to all 'edge' hexes, and traversing that line of hexes
 * marking each hex as 'seen' and as 'los-or-not'. For each sensor on the
 * 'mech, we keep track of how steep the angle has to be in order for the
 * sensor to 'see' the terrain. The start angle is -20 (which should be low
 * enough for common purposes, even on jumping 'mechs) for sensors that can
 * see terrain, and 1000 for sensors that can't -- basically flagging the
 * whole line of sight as 'not seen' for that sensor.

 * In order to take wood-blockage into account, we also keep track of the
 * minimum 'block' angle. That is, if it is not equal to minangle,
 * blockangle is the angle below which 'woodcount' woods stand between the
 * current hex and the seeing 'mech. If we end up with a hex between
 * minangle and blockangle, we need to check if the sensor can see through
 * that many woods.

 * Blocking entirely, because of water- or EM-effects, is done by setting
 * the minangle and blockangle to 1000, a value high enough to block los to
 * all following hexes. To determine whether a sensors sees through a hex,
 * fake losflags are passed to the regular sensor functions... hacks, and
 * logic-duplication (the worst kind) but they work for now.

 * This is all proof-of-concept, based on Cord Awtry's ideas for
 * 'underground' maps. This should all be rewritten, together with the
 * sensor code, to have one general 'tracelos' function, which calls
 * callbacks defined on a state struct and stores its state info on that
 * same struct. That way, map-los, 'mech-los, searchlight-los and such can
 * all use the same routine, using different callbacks.

 * Known bugs / problems:
 * - It behaves awkwardly around water. It doesn't handle the transition as
 *   it should. This requires sufficient rewriting that I do not plan to do
 *   it before the whole sensor overhaul.

 * - It has too great a leniency in what terrain height you can see. You can
 *   sometimes see a level 1 hex behind a level 2 hex if you are in a 'mech,
 *   fallen on a level 1 hex. (you shouldn't.)

 */

typedef struct SliteTraceRequest {
  HexLosMap *los_map;
  BattleMap *map;
  Mech *mech;
  int index;
  float start_height;
  LosTrace *trace;
} SliteTraceRequest;

static void trace_slitelos(const SliteTraceRequest *request) {
  HexLosMap *los_map = request->los_map;
  BattleMap *map = request->map;
  Mech *mech = request->mech;
  float minangle = -20;
  int trace_range = 0;
  int trace_x;
  int trace_y;
  int trace_height;
  float trace_a;
  int trace_coordnum =
      trace_los(map, mech_position_x(mech), mech_position_y(mech),
                los_map_index_x(los_map, request->index),
                los_map_index_y(los_map, request->index), request->trace);

  for (; trace_range < trace_coordnum; trace_range++) {
    const LosTracePoint *point = los_trace_point(request->trace, trace_range);
    trace_x = point->x;
    trace_y = point->y;

    trace_height = max(0, map_elevation_get(map, trace_x, trace_y));

    if (!mech_searchlight_reaches(
            &(SearchlightReachRequest){.mech = mech,
                                       .position = {.x = trace_x, .y = trace_y},
                                       .elevation = trace_height}))
      return;

    trace_a = ((float)trace_height - request->start_height) /
              (float)(trace_range + 1);
    switch (map_terrain_get(map, trace_x, trace_y)) {
    case HEAVY_FOREST:
    case LIGHT_FOREST:
    case SMOKE:
      trace_a += 2;
    }

    if (trace_a < minangle)
      continue;

    set_sliteinfo(los_map, trace_x, trace_y, MAPLOSHEX_LIT);
    minangle = trace_a;
  }
}

static void litemark_callback(BattleMap *map, int x, int y, void *context) {
  (void)map;
  set_sliteinfo(context, x, y, MAPLOSHEX_LIT);
}

static void litemark_map(HexLosMap *los_map, BattleMap *map, LosTrace *trace) {
  Mech *mech;
  int i;
  int index;
  MapObject *fire;

  for (fire = first_mapobj(map, TYPE_FIRE); fire; fire = next_mapobj(fire)) {
    set_sliteinfo(los_map, fire->x, fire->y, MAPLOSHEX_LIT);
    visit_neighbor_hexes(map, fire->x, fire->y, litemark_callback, los_map);
  }

  for (i = 0; i < battle_map_unit_count(map); i++) {
    const DbRef UNIT_DBREF = battle_map_unit_dbref(map, i);
    if (UNIT_DBREF < 0)
      continue;
    mech = btech_context_get_mech(map->xcode.context, UNIT_DBREF);
    if (!mech)
      continue;

    if (mech_is_jellied(mech)) {
      set_sliteinfo(los_map, mech_position_x(mech), mech_position_y(mech),
                    MAPLOSHEX_LIT);
      visit_neighbor_hexes(map, mech_position_x(mech), mech_position_y(mech),
                           litemark_callback, los_map);
    }

    if (!mech_searchlight_active(mech))
      continue;

    for (index = 0; index < los_map->xsize * los_map->ysize; index++) {
      const int MECH_Z = mech_position_z(mech);
      const float LIGHT_HEIGHT = (float)MECH_Z + mech_los_height(mech);
      trace_slitelos(&(SliteTraceRequest){.los_map = los_map,
                                          .map = map,
                                          .mech = mech,
                                          .index = index,
                                          .start_height = LIGHT_HEIGHT,
                                          .trace = trace});
    }
  }
}

static float default_minimum_angle(Mech *mech, int sensor) {
  return mech_sensor_sees_terrain(mech, sensor) ? -20.0F : 1000.0F;
}

typedef struct SensorTraceState {
  int trace_water;
  float minimum_angle;
  float block_angle;
  int wood_count;
  int water_count;
} SensorTraceState;

static SensorTraceState *sensor_trace_state(SensorTraceState *states,
                                            int sensor) {
  if (sensor < 0)
    abort();
  return checked_storage_at(states, MAX_SENSORS, sizeof(*states),
                            (size_t)sensor);
}

typedef struct MapHexTraceRequest {
  HexLosMap *los_map;
  BattleMap *map;
  Mech *mech;
  int index;
  bool trace_water;
  float start_height;
  LosTrace *trace;
} MapHexTraceRequest;

static void trace_maphexlos(const MapHexTraceRequest *request) {
  HexLosMap *los_map = request->los_map;
  BattleMap *map = request->map;
  Mech *mech = request->mech;
  SensorTraceState states[MAX_SENSORS] = {
      {.trace_water = request->trace_water,
       .minimum_angle = default_minimum_angle(mech, 0),
       .block_angle = default_minimum_angle(mech, 0)},
      {.trace_water = request->trace_water,
       .minimum_angle = default_minimum_angle(mech, 1),
       .block_angle = default_minimum_angle(mech, 1)},
  };
  int trace_range = 0;

  int trace_coordnum =
      trace_los(map, mech_position_x(mech), mech_position_y(mech),
                los_map_index_x(los_map, request->index),
                los_map_index_y(los_map, request->index), request->trace);

  for (; trace_range < trace_coordnum; trace_range++) {
    int seestate;
    const LosTracePoint *point = los_trace_point(request->trace, trace_range);
    int trace_x = point->x;
    int trace_y = point->y;
    int trace_height = (unsigned char)map_elevation_get(map, trace_x, trace_y);

    float trace_a = ((float)trace_height - request->start_height) /
                    (float)(trace_range + 1);
    float trace_ba = ((float)(trace_height + 2) - request->start_height) /
                     (float)(trace_range + 1);
    int trace_terrain =
        (unsigned char)map_real_terrain_get(map, trace_x, trace_y);
    int nsensor;
    int newwoods;

    for (nsensor = 0; nsensor < MAX_SENSORS; nsensor++) {
      SensorTraceState *state = sensor_trace_state(states, nsensor);

      /* If the current hex and all its terrain ('blockangle') lies
       * below our minimum angle of sight, we won't see it at all;
       * jump straight ahead to the water/mountain checks. This check
       * is made first, because it is the cheapest check and the
       * general mechanism to signal 'no more visibility on this line
       * of sight' is to set trace_ba to an impossible angle.
       */

      if (trace_ba < state->minimum_angle) {
        set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_NOLOS);
        goto hexinfluence;
      }

      /* Then we check for range. */
      seestate = mech_los_sees_range(
          &(SensorRangeRequest){.los_map = los_map,
                                .mech = mech,
                                .map = map,
                                .position = {.x = trace_x, .y = trace_y},
                                .elevation = trace_height,
                                .sensor = nsensor});

      if (seestate == 0) {
        set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_NOLOS);
        state->minimum_angle = state->block_angle = 1000;
        goto hexinfluence;
      }

      /* Count the number of woods. */
      newwoods = 0;
      switch (trace_terrain) {
      case HEAVY_FOREST:
        newwoods++;
        [[fallthrough]];
      case LIGHT_FOREST:
        newwoods++;
        /* Because we aren't in water, we stop tracing below water */
        state->trace_water = 0;
        break;
      }

      if (!newwoods) {

        if (trace_a < state->minimum_angle ||
            (seestate < 0 && !hexlit(los_map, trace_x, trace_y))) {
          set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_NOLOS);
        } else {
          set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_SEE);
          state->block_angle = state->minimum_angle = trace_a;
          state->wood_count = 0;
        }
        goto hexinfluence;
      }

      if (state->block_angle < trace_a) {
        state->minimum_angle = trace_a;
        state->block_angle = trace_ba;
        state->wood_count = newwoods;
      } else if (!mech_los_sees_through_obstruction(&(SensorObstructionRequest){
                     .mech = mech,
                     .map = map,
                     .count = state->wood_count + newwoods,
                     .sensor = nsensor,
                     .los_flag = BATTLE_MAP_LOS_WOOD})) {
        if (trace_ba >= state->block_angle) {
          state->minimum_angle = state->block_angle;
          state->block_angle = trace_ba;
          state->wood_count = newwoods;
        } else {
          state->minimum_angle = trace_ba;
        }
      } else {
        state->minimum_angle = trace_a;
        state->wood_count += newwoods;
      }

      if (trace_terrain == WATER) {
        if (state->trace_water)
          state->water_count++;
        if (!state->trace_water ||
            !mech_los_sees_through_obstruction(&(SensorObstructionRequest){
                .mech = mech,
                .map = map,
                .count = state->water_count,
                .sensor = nsensor,
                .los_flag = BATTLE_MAP_LOS_WATER})) {
          if (seestate < 0 && !hexlit(los_map, trace_x, trace_y))
            set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_NOLOS);
          else
            set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_SEETERRAIN);
        }
      } else if (seestate < 0 && !hexlit(los_map, trace_x, trace_y)) {
        set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_NOLOS);
      } else {
        set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_SEE);
      }

    hexinfluence:
      if (trace_terrain == WATER &&
          !mech_los_sees_through_obstruction(
              &(SensorObstructionRequest){.mech = mech,
                                          .map = map,
                                          .count = 1,
                                          .sensor = nsensor,
                                          .los_flag = BATTLE_MAP_LOS_WATER})) {
        set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_NOLOS);
        state->minimum_angle = state->block_angle = 1000;
        continue;
      }
      state->trace_water = 0;
      if (trace_terrain == MOUNTAINS &&
          !mech_los_sees_over_mountain(mech, map, nsensor)) {
        set_hexlosinfo(los_map, trace_x, trace_y, MAPLOSHEX_NOLOS);
        state->minimum_angle = state->block_angle = 1000;
        continue;
      }
    }
  }
}

bool los_map_calculate(HexLosMap *los_map, BattleMap *map, Mech *mech, int sx,
                       int sy, int xsz, int ysz) {
  int index;
  int underterrain;
  int bothworlds;
  float start_height;
  LosTrace trace;

  /* Some safeguarding on size */

  if (xsz < 1 || ysz < 1 || xsz > MAPLOS_MAXX || ysz > MAPLOS_MAXY) {
    btech_channel_send(map->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       tprintf("xsize (%d vs %d) or ysize (%d vs %d) "
                               "to CalculateLOSMap too large, for mech #%ld",
                               xsz, MAPLOS_MAXX, ysz, MAPLOS_MAXY,
                               mech_dbref(mech)));
    return false;
  }

  *los_map = (HexLosMap){
      .context = map->xcode.context,
      .startx = sx,
      .starty = sy,
      .xsize = xsz,
      .ysize = ysz,
  };

  underterrain = mech_position_z(mech) <= -1;
  int terrain = (unsigned char)mech_real_terrain_get(mech);
  int movement = mech_movement_type(mech);
  if ((terrain == ICE || terrain == WATER || terrain == BRIDGE) &&
      ((mech_class(mech) == CLASS_MECH && mech_position_z(mech) == -1) ||
       ((movement == MOVE_HULL || movement == MOVE_FOIL ||
         movement == MOVE_HOVER) &&
        mech_position_z(mech) == 0))) {
    bothworlds = 1;
  } else {
    bothworlds = 0;
  }

  const int MECH_Z = mech_position_z(mech);
  start_height = (float)MECH_Z + mech_los_height(mech);

  if (mech_is_clairvoyant(mech)) {
    set_hexlosall(los_map, MAPLOSHEX_SEE);
    return true;
  }

  if (!mech_sensor_sees_terrain(mech, 0) &&
      !mech_sensor_sees_terrain(mech, 1)) {
    set_hexlosall(los_map, MAPLOSHEX_NOLOS);
    return true;
  }

  /* In order for slites to properly light terrain, we have to mark the
   * losmap with all lit hexes first. Which means going over all 'mechs on
   * the map and tag all hexes that they light.
   */

  litemark_map(los_map, map, &trace);

  /* In order to do the most efficient lostracing, we make losmaps by
   * first tracing from the 'mech hex to the upper Y-row, the lower Y-row,
   * the leftmost X-row, the rightmost X-row, and then all hexes starting
   * at the upper left corner to make sure we have seen all hexes. (It is
   * entirely possible for a hex not to be visited yet, even if we traced
   * to every other hex.)
   */

  for (index = 0; index < xsz; index++) {
    if (*los_map_cell(los_map, index) & MAPLOSHEX_SEEN)
      continue;
    trace_maphexlos(
        &(MapHexTraceRequest){.los_map = los_map,
                              .map = map,
                              .mech = mech,
                              .index = index,
                              .trace_water = underterrain || bothworlds,
                              .start_height = start_height,
                              .trace = &trace});
  }
  for (index = (ysz - 1) * xsz; index < ysz * xsz; index++) {
    if (*los_map_cell(los_map, index) & MAPLOSHEX_SEEN)
      continue;
    trace_maphexlos(
        &(MapHexTraceRequest){.los_map = los_map,
                              .map = map,
                              .mech = mech,
                              .index = index,
                              .trace_water = underterrain || bothworlds,
                              .start_height = start_height,
                              .trace = &trace});
  }
  for (index = xsz; index < ysz * xsz; index += xsz) {
    if (*los_map_cell(los_map, index) & MAPLOSHEX_SEEN)
      continue;
    trace_maphexlos(
        &(MapHexTraceRequest){.los_map = los_map,
                              .map = map,
                              .mech = mech,
                              .index = index,
                              .trace_water = underterrain || bothworlds,
                              .start_height = start_height,
                              .trace = &trace});
  }
  for (index = 2 * xsz - 1; index < ysz * xsz; index += xsz) {
    if (*los_map_cell(los_map, index) & MAPLOSHEX_SEEN)
      continue;
    trace_maphexlos(
        &(MapHexTraceRequest){.los_map = los_map,
                              .map = map,
                              .mech = mech,
                              .index = index,
                              .trace_water = underterrain || bothworlds,
                              .start_height = start_height,
                              .trace = &trace});
  }
  for (index = 0; index < xsz * ysz; index++) {
    if (*los_map_cell(los_map, index) & MAPLOSHEX_SEEN)
      continue;
    trace_maphexlos(
        &(MapHexTraceRequest){.los_map = los_map,
                              .map = map,
                              .mech = mech,
                              .index = index,
                              .trace_water = underterrain || bothworlds,
                              .start_height = start_height,
                              .trace = &trace});
  }

  return true;
}
