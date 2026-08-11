/* Implements BattleTech movement mechanics for unit update speed. */

#include "mech_update_api.h"

#include <math.h>

#include "btech/context.h"
#include "btechstats_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "registry_api.h"
#include "section_types.h"

static float speed_heat_decrease(float speed, float maximum_speed,
                                 float penalty) {
  return speed *
         (ceilf((rintf((maximum_speed / 1.5F) / MP1) - (penalty / MP1)) *
                1.5F) *
          MP1) /
         maximum_speed;
}

static float speed_old_decrease(float speed, float maximum_speed,
                                float penalty) {
  return speed * (maximum_speed - penalty) / maximum_speed;
}

static float speed_old_increase(float speed, float maximum_speed, float bonus) {
  return speed_old_decrease(speed, maximum_speed, -bonus);
}

static float speed_new_decrease(float speed, float penalty) {
  return speed * MP1 / (MP1 + penalty);
}

/* If you want to simulate _OLDs, you have to add 1MP in some cases (eww) */

float mech_terrain_speed(const MechTerrainSpeedRequest *request) {
  Mech *mech = request->mech;
  float tempspeed = request->current_speed;
  const float MAXSPEED = request->maximum_speed;
  switch (request->terrain) {
  case BATTLE_TERRAIN_SNOW:
  case BATTLE_TERRAIN_ROUGH:
    tempspeed = speed_new_decrease(tempspeed, MP1);
    break;
  case BATTLE_TERRAIN_MOUNTAINS:
    tempspeed = speed_new_decrease(tempspeed, MP2);
    break;
  case BATTLE_TERRAIN_LIGHT_FOREST:
    if (mech_class(mech) != CLASS_BSUIT)
      tempspeed = speed_new_decrease(tempspeed, MP1);
    break;
  case BATTLE_TERRAIN_HEAVY_FOREST:
    if (mech_class(mech) != CLASS_BSUIT)
      tempspeed = speed_new_decrease(tempspeed, MP2);
    break;
  case BATTLE_TERRAIN_BRIDGE:
  case BATTLE_TERRAIN_ROAD:
    /* Ground units (wheeled and tracked) get +1 MP moving on paved surface */
#ifndef BT_MOVEMENT_MODES
    if (mech_movement_type(mech) == MOVE_TRACK ||
        mech_movement_type(mech) == MOVE_WHEEL)
#else
    if (!mech_condition_summary(mech).sprinting &&
        (mech_movement_type(mech) == MOVE_TRACK ||
         mech_movement_type(mech) == MOVE_WHEEL))
#endif
      tempspeed = speed_old_increase(tempspeed, MAXSPEED, MP1);
    [[fallthrough]];
  case BATTLE_TERRAIN_ICE:
    if (mech_position_z(mech) >= 0)
      break;
    /* if he's under the ice/bridge, treat as water. */
    [[fallthrough]];
  case BATTLE_TERRAIN_WATER:
    if (mech_movement_type(mech) == MOVE_BIPED ||
        mech_movement_type(mech) == MOVE_QUAD) {
      if (request->elevation <= -2)
        tempspeed = speed_new_decrease(tempspeed, MP3);
      else if (request->elevation == -1)
        tempspeed = speed_new_decrease(tempspeed, MP1);
    }
    break;
  }
  return tempspeed;
}

void mech_speed_update(Mech *mech) {
  float acc, tempspeed, maxspeed;
  Mech *target;
  BtechContext *context = mech_context(mech);
  MechConditionSummary conditions = mech_condition_summary(mech);
  int technology = mech_technology_flags(mech);

  if (mech_is_fallen(mech) || mech_is_jumping(mech) ||
      mech_maximum_speed(mech) <= 0.0F)
    return;
  tempspeed = fabsf(mech_desired_speed(mech));
  maxspeed = mech_effective_maximum_speed(mech);
  if (maxspeed < 0.0F)
    maxspeed = 0.0F;

  if (conditions.masc_enabled && conditions.supercharger_enabled)
    maxspeed = ceilf((rintf(maxspeed / 1.5F) / MP1) * 2.5F) * MP1;
  else if (conditions.masc_enabled)
    maxspeed = (4.0F / 3.0F) * maxspeed;
  else if (conditions.supercharger_enabled)
    maxspeed = (4.0F / 3.0F) * maxspeed;

  if (technology & TRIPLE_MYOMER_TECH) {
    if (mech_excess_heat(mech) >= 9.0F)
      maxspeed =
          ceilf((rintf((mech_effective_maximum_speed(mech) / 1.5F) / MP1) +
                 1.0F) *
                1.5F) *
          MP1;
    if (mech_desired_speed(mech) >= maxspeed)
      mech_desired_speed_set(mech, maxspeed);
  }

  if (mech_excess_heat(mech) >= 5.0F) {
    if (mech_excess_heat(mech) >= 25.0F)
      tempspeed = speed_heat_decrease(tempspeed, maxspeed, MP5);
    else if (mech_excess_heat(mech) >= 20.0F)
      tempspeed = speed_heat_decrease(tempspeed, maxspeed, MP4);
    else if (mech_excess_heat(mech) >= 15.0F)
      tempspeed = speed_heat_decrease(tempspeed, maxspeed, MP3);
    else if (mech_excess_heat(mech) >= 10.0F)
      tempspeed = speed_heat_decrease(tempspeed, maxspeed, MP2);
    else if (!((technology & TRIPLE_MYOMER_TECH) &&
               mech_excess_heat(mech) >= 9.0F))
      tempspeed = speed_heat_decrease(tempspeed, maxspeed, MP1);
  }
  if (mech_class(mech) != CLASS_MW && mech_movement_type(mech) != MOVE_VTOL &&
      (mech_movement_type(mech) != MOVE_FLY || mech_is_landed(mech)))
    tempspeed = mech_terrain_speed(&(MechTerrainSpeedRequest){
        .mech = mech,
        .current_speed = tempspeed,
        .maximum_speed = maxspeed,
        .terrain = mech_real_terrain_get(mech),
        .elevation = mech_position_elevation(mech),
    });
  if (mech_heading_changed(mech)) {
    if (btech_context_movement_slowdown_mode(context) == 2) {
      int dif = mech_heading_degrees(mech) - mech_desired_heading_degrees(mech);

      if (dif < 0)
        dif = -dif;
      if (dif > 180)
        dif = 360 - dif;
      if (dif) {
        dif = (dif - 1) / 30;
        dif = (dif + 2);
        if (conditions.tight_turn_mode)
          tempspeed = (tempspeed * (float)(10 - dif) / 10.0F) - (MP1 * 0.4F);
        else
          tempspeed = tempspeed * (float)(10 - dif) / 10.0F;
      }
    } else if (btech_context_movement_slowdown_mode(context) == 1) {
      if (mech_heading_degrees(mech) != mech_desired_heading_degrees(mech))
        tempspeed = tempspeed * 2.0F / 3.0F;
      else
        tempspeed = tempspeed * 3.0F / 4.0F;
    }
#ifdef BT_MOVEMENT_MODES
    if ((conditions.sprinting || conditions.evading) &&
        !(has_bool_advantage(context, mech_pilot_dbref(mech), "speed_demon") ||
          has_bool_advantage(context, mech_pilot_dbref(mech),
                             "maneuvering_ace")))
      tempspeed = (tempspeed * 2.0F) / 3.0F;
#endif
    mech_heading_change_clear(mech);
  }
  if (mech_movement_type(mech) == MOVE_QUAD && mech_lateral_movement(mech))
    tempspeed = speed_old_decrease(tempspeed, maxspeed, MP1);
#ifdef BT_MOVEMENT_MODES
  else if (mech_lateral_movement(mech)) {
    if (has_bool_advantage(context, mech_pilot_dbref(mech), "maneuvering_ace"))
      tempspeed = speed_old_decrease(tempspeed, maxspeed, MP2);
    else
      tempspeed = speed_old_decrease(tempspeed, maxspeed, MP3);
  }
#endif
  if (tempspeed <= 0.0F)
    tempspeed = 0.0F;
  if (mech_desired_speed(mech) < 0.0F)
    tempspeed = -tempspeed;

  float current_speed = mech_current_speed(mech);
  if (fabsf(tempspeed - current_speed) > 0.0001F) {
    if (mech_movement_type(mech) == MOVE_QUAD)
      acc = maxspeed / 10.0F;
    else
      acc = maxspeed / 20.0F;
    if (has_bool_advantage(context, mech_pilot_dbref(mech), "speed_demon"))
      acc *= 1.25F;

    if (tempspeed < current_speed) {
      current_speed -= acc;
      if (tempspeed > current_speed)
        current_speed = tempspeed;
    } else {
      current_speed += acc;
      if (tempspeed < current_speed)
        current_speed = tempspeed;
    }
    mech_current_speed_set(mech, current_speed);
  }
  if (mech_carried_dbref(mech) > 0) {
    target = btech_context_get_mech(context, mech_carried_dbref(mech));
    if (target)
      mech_current_speed_set(target, mech_current_speed(mech));
  }
}
