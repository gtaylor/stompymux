#include "ai_simulation_api.h"

#include <assert.h>
#include <limits.h>
#include <math.h>
#include <stdlib.h>

#include "btconfig.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"

static short simulation_map_coordinate(int coordinate) {
  assert(coordinate >= SHRT_MIN && coordinate <= SHRT_MAX);
  return (short)coordinate;
}

static bool ai_section_is_floodable(Mech *mech, int section) {
  return mech_section_internal(mech, section) &&
         (!mech_section_armor(mech, section) ||
          (!mech_section_rear_armor(mech, section) &&
           mech_section_original_rear_armor(mech, section)));
}

static int ai_map_elevation(BattleMap *map, int x, int y) {
  const int elevation = map_elevation_get(map, x, y);
  const char terrain = map_real_terrain_get(map, x, y);
  return terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
             ? -elevation
             : elevation;
}

int ai_crash(BattleMap *map, Mech *mech, LocationSimulation *location) {
  if (map == nullptr)
    return 0;

  float maximum_speed = mech_effective_maximum_speed(mech);
  bool heading_changed = false;
  if (location->h != location->dh && !mech_has_destroyed_gyro(mech)) {
    heading_changed = true;
    if (mech_is_aerospace_unit(mech))
      maximum_speed *= ACCEL_MOD;
    int normalized_angle = location->h - location->dh;
    const int movement_modifier =
        mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT ? 60
                                                                        : 1;
    float offset_value;
    if (fabsf(location->s) < 1.0F) {
      offset_value =
          3.0F * maximum_speed * MP_PER_KPH * (float)movement_modifier;
    } else {
      offset_value =
          2.0F * maximum_speed * MP_PER_KPH * (float)movement_modifier;
      if (abs(normalized_angle) > (int)offset_value &&
          location->s > maximum_speed * 2.0F / 3.0F)
        offset_value -=
            offset_value / 2.0F * (3.0F * location->s / maximum_speed - 2.0F);
    }
    int offset = (int)(offset_value * (float)MOVE_MOD);
    if (normalized_angle < 0)
      normalized_angle += 360;
    if (mech_is_dropship(mech) && offset >= 10)
      offset = 10;
    if (normalized_angle > 180) {
      location->h += offset;
      if (location->h >= 360)
        location->h -= 360;
      normalized_angle += offset;
      if (normalized_angle >= 360)
        location->h = location->dh;
    } else {
      location->h -= offset;
      if (location->h < 0)
        location->h += 360;
      normalized_angle -= offset;
      if (normalized_angle < 0)
        location->h = location->dh;
    }
  }

  float target_speed = location->ds;
  if (mech_class(mech) != CLASS_MW && mech_movement_type(mech) != MOVE_VTOL &&
      (mech_movement_type(mech) != MOVE_FLY || mech_is_landed(mech)))
    target_speed = mech_terrain_speed(mech, target_speed, maximum_speed,
                                      location->t, location->e);
  if (heading_changed) {
    const int slowdown =
        btech_context_movement_slowdown_mode(mech_context(mech));
    if (slowdown == 2) {
      int difference =
          mech_heading_degrees(mech) - mech_desired_heading_degrees(mech);
      if (difference < 0)
        difference = -difference;
      if (difference > 180)
        difference = 360 - difference;
      if (difference) {
        difference = (difference - 1) / 30 + 2;
        target_speed = target_speed * (float)(10 - difference) / 10.0F;
      }
    } else if (slowdown == 1) {
      target_speed *= location->h != location->dh ? 2.0F / 3.0F : 3.0F / 4.0F;
    }
  }
  if (mech_movement_type(mech) == MOVE_QUAD && mech_lateral_movement(mech))
    target_speed -= MP1;
  if (target_speed <= 0.0F)
    target_speed = 0.0F;
  if (location->ds < 0.0F)
    target_speed = -target_speed;
  if (fabsf(target_speed - location->s) > 0.0001F) {
    const float acceleration =
        maximum_speed / (mech_movement_type(mech) == MOVE_QUAD ? 10.0F : 20.0F);
    if (target_speed < location->s) {
      location->s -= acceleration;
      if (target_speed > location->s)
        location->s = target_speed;
    } else {
      location->s += acceleration;
      if (target_speed < location->s)
        location->s = target_speed;
    }
  }

  float x_delta = 0.0F;
  float y_delta = 0.0F;
  FindComponents(location->s * (float)MOVE_MOD, location->h, &x_delta,
                 &y_delta);
  location->fx += x_delta;
  location->fy += y_delta;
  location->lx = location->x;
  location->ly = location->y;
  RealCoordToMapCoord(&location->x, &location->y, location->fx, location->fy);
  if (BOUNDED(0, location->x, battle_map_width(map) - 1) != location->x ||
      BOUNDED(0, location->y, battle_map_height(map) - 1) != location->y)
    return 1;
  if (location->lx == location->x && location->ly == location->y)
    return 0;

  const int old_elevation = location->e;
  switch (map_real_terrain_get(map, location->x, location->y)) {
  case BATTLE_TERRAIN_HEAVY_FOREST:
    if (mech_class(mech) != CLASS_MECH)
      return 1;
    break;
  case BATTLE_TERRAIN_WATER:
    if (mech_movement_type(mech) == MOVE_TRACK ||
        mech_movement_type(mech) == MOVE_WHEEL)
      return 1;
    if (mech_class(mech) == CLASS_MECH) {
      const int elevation = ai_map_elevation(map, location->x, location->y);
      if (elevation == -1) {
        if (ai_section_is_floodable(mech, LLEG) ||
            ai_section_is_floodable(mech, RLEG) ||
            (mech_movement_type(mech) == MOVE_QUAD &&
             (ai_section_is_floodable(mech, LARM) ||
              ai_section_is_floodable(mech, RARM))))
          return 1;
      } else if (elevation < -1) {
        for (int section = 0; section < NUM_SECTIONS; section++) {
          if (ai_section_is_floodable(mech, section))
            return 1;
        }
      }
    }
    break;
  case BATTLE_TERRAIN_HIGH_WATER:
    return 1;
  }

  location->e = ai_map_elevation(map, location->x, location->y);
  if (mech_movement_type(mech) == MOVE_HOVER)
    location->e = MAX(0, location->e);
  location->t = map_real_terrain_get(map, location->x, location->y);
  if (mech_class(mech) == CLASS_MECH && (abs(location->e - old_elevation) > 2))
    return 1;
  if (mech_class(mech) == CLASS_VEH_GROUND &&
      (abs(location->e - old_elevation) > 1))
    return 1;
  return 0;
}

void location_simulation_initialize(LocationSimulation *location, Mech *mech) {
  location->fx = mech_position_real_x(mech);
  location->fy = mech_position_real_y(mech);
  const char terrain = mech_real_terrain_get(mech);
  const int elevation = mech_position_elevation_magnitude(mech);
  location->e = terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
                    ? -elevation
                    : elevation;
  location->h = mech_heading_degrees(mech);
  location->dh = mech_desired_heading_degrees(mech);
  location->s = mech_current_speed(mech);
  location->t = terrain;
  location->ds = mech_desired_speed(mech);
  location->x = simulation_map_coordinate(mech_position_x(mech));
  location->y = simulation_map_coordinate(mech_position_y(mech));
  location->lx = location->x;
  location->ly = location->y;
}
