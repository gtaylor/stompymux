#include "ai_simulation_api.h"

#include <assert.h>
#include <limits.h>
#include <math.h>
#include <stdlib.h>

#include "btconfig.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "section_types.h"

static short simulation_map_coordinate(int coordinate) {
  assert(coordinate >= SHRT_MIN && coordinate <= SHRT_MAX);
  return (short)coordinate;
}

static bool ai_section_is_floodable(Mech *mech, int section) {
  return (mech_section_internal(mech, section) &&
          (!mech_section_armor(mech, section) ||
           (!mech_section_rear_armor(mech, section) &&
            mech_section_original_rear_armor(mech, section)))) != 0;
}

static int ai_map_elevation(BattleMap *map, int x, int y) {
  const int ELEVATION = (unsigned char)map_elevation_get(map, x, y);
  const char TERRAIN = map_real_terrain_get(map, x, y);
  return TERRAIN == BATTLE_TERRAIN_WATER || TERRAIN == BATTLE_TERRAIN_ICE
             ? -ELEVATION
             : ELEVATION;
}

bool ai_crash(BattleMap *map, Mech *mech, LocationSimulation *location) {
  if (map == nullptr)
    return false;

  float maximum_speed = mech_effective_maximum_speed(mech);
  bool heading_changed = false;
  if (location->h != location->dh && !mech_has_destroyed_gyro(mech)) {
    heading_changed = true;
    if (mech_is_aerospace_unit(mech))
      maximum_speed *= ACCEL_MOD;
    int normalized_angle = location->h - location->dh;
    const int MOVEMENT_MODIFIER =
        mech_class(mech) == CLASS_MW || mech_class(mech) == CLASS_BSUIT ? 60
                                                                        : 1;
    float offset_value;
    if (fabsf(location->s) < 1.0F) {
      offset_value =
          3.0F * maximum_speed * MP_PER_KPH * (float)MOVEMENT_MODIFIER;
    } else {
      offset_value =
          2.0F * maximum_speed * MP_PER_KPH * (float)MOVEMENT_MODIFIER;
      if (abs(normalized_angle) > (int)offset_value &&
          location->s > maximum_speed * 2.0F / 3.0F)
        offset_value -=
            offset_value / 2.0F * ((3.0F * location->s / maximum_speed) - 2.0F);
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
      (mech_movement_type(mech) != MOVE_FLY || mech_is_landed(mech))) {
    target_speed = mech_terrain_speed(&(MechTerrainSpeedRequest){
        .mech = mech,
        .current_speed = target_speed,
        .maximum_speed = maximum_speed,
        .terrain = location->t,
        .elevation = location->e,
    });
  }
  if (heading_changed) {
    const int SLOWDOWN =
        btech_context_movement_slowdown_mode(mech_context(mech));
    if (SLOWDOWN == 2) {
      int difference =
          mech_heading_degrees(mech) - mech_desired_heading_degrees(mech);
      if (difference < 0)
        difference = -difference;
      if (difference > 180)
        difference = 360 - difference;
      if (difference) {
        difference = ((difference - 1) / 30) + 2;
        target_speed = target_speed * (float)(10 - difference) / 10.0F;
      }
    } else if (SLOWDOWN == 1) {
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
    const float ACCELERATION =
        maximum_speed / (mech_movement_type(mech) == MOVE_QUAD ? 10.0F : 20.0F);
    if (target_speed < location->s) {
      location->s -= ACCELERATION;
      if (target_speed > location->s)
        location->s = target_speed;
    } else {
      location->s += ACCELERATION;
      if (target_speed < location->s)
        location->s = target_speed;
    }
  }

  MapRealPosition delta = map_vector_components(&(MapPolarVector){
      .magnitude = location->s * (float)MOVE_MOD, .bearing = location->h});
  location->fx += delta.x;
  location->fy += delta.y;
  location->lx = location->x;
  location->ly = location->y;
  real_coord_to_map_coord(&location->x, &location->y, location->fx,
                          location->fy);
  if (bounded(0, location->x, battle_map_width(map) - 1) != location->x ||
      bounded(0, location->y, battle_map_height(map) - 1) != location->y)
    return true;
  if (location->lx == location->x && location->ly == location->y)
    return false;

  const int OLD_ELEVATION = location->e;
  switch (map_real_terrain_get(map, location->x, location->y)) {
  case BATTLE_TERRAIN_HEAVY_FOREST:
    if (mech_class(mech) != CLASS_MECH)
      return true;
    break;
  case BATTLE_TERRAIN_WATER:
    if (mech_movement_type(mech) == MOVE_TRACK ||
        mech_movement_type(mech) == MOVE_WHEEL)
      return true;
    if (mech_class(mech) == CLASS_MECH) {
      const int ELEVATION = ai_map_elevation(map, location->x, location->y);
      if (ELEVATION == -1) {
        if (ai_section_is_floodable(mech, LLEG) ||
            ai_section_is_floodable(mech, RLEG) ||
            (mech_movement_type(mech) == MOVE_QUAD &&
             (ai_section_is_floodable(mech, LARM) ||
              ai_section_is_floodable(mech, RARM))))
          return true;
      } else if (ELEVATION < -1) {
        for (int section = 0; section < NUM_SECTIONS; section++) {
          if (ai_section_is_floodable(mech, section))
            return true;
        }
      }
    }
    break;
  case BATTLE_TERRAIN_HIGH_WATER:
    return true;
  }

  location->e = ai_map_elevation(map, location->x, location->y);
  if (mech_movement_type(mech) == MOVE_HOVER)
    location->e = max(0, location->e);
  location->t =
      (unsigned char)map_real_terrain_get(map, location->x, location->y);
  if (mech_class(mech) == CLASS_MECH && (abs(location->e - OLD_ELEVATION) > 2))
    return true;
  if (mech_class(mech) == CLASS_VEH_GROUND &&
      (abs(location->e - OLD_ELEVATION) > 1))
    return true;
  return false;
}

void location_simulation_initialize(LocationSimulation *location, Mech *mech) {
  location->fx = mech_position_real_x(mech);
  location->fy = mech_position_real_y(mech);
  const char TERRAIN = mech_real_terrain_get(mech);
  const int ELEVATION = mech_position_elevation_magnitude(mech);
  location->e = TERRAIN == BATTLE_TERRAIN_WATER || TERRAIN == BATTLE_TERRAIN_ICE
                    ? -ELEVATION
                    : ELEVATION;
  location->h = mech_heading_degrees(mech);
  location->dh = mech_desired_heading_degrees(mech);
  location->s = mech_current_speed(mech);
  location->t = (unsigned char)TERRAIN;
  location->ds = mech_desired_speed(mech);
  location->x = simulation_map_coordinate(mech_position_x(mech));
  location->y = simulation_map_coordinate(mech_position_y(mech));
  location->lx = location->x;
  location->ly = location->y;
}
