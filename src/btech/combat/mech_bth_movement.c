
/* Calculates movement modifiers for combat base-to-hit values. */

#include <math.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_c3_misc_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"
int mech_attacker_movement_modifier(Mech *mech) {
  float maxspeed;
  float speed;
  int base = 0;

  if (mech_class(mech) == CLASS_BSUIT)
    return 0;

  maxspeed = mech_template_maximum_speed(mech);
  if ((mech_excess_heat(mech) >= 9.0F) &&
      (mech_technology_flags(mech) & TRIPLE_MYOMER_TECH))
    maxspeed += 1.5F * MP1;
  if (mech_is_jumping(mech))
    return 3;

  /* quads don't suffer the +2 BTH firing while prone if they have all 4 legs */
  if ((!mech_is_quad(mech) ||
       (mech_is_quad(mech) && CountDestroyedLegs(mech) > 0)) &&
      mech_condition_summary(mech).fallen && !mech_is_dropship(mech))
    return 2;

  if (!mech_is_jumping(mech) && (mech_event_count(mech, EVENT_JUMPSTABIL) ||
                                 mech_event_count(mech, EVENT_STAND)))
    return 2;

  speed = mech_current_speed(mech);

  if (btech_context_uses_fasa_turning(mech_context(mech)))
    if (mech_heading_degrees(mech) != mech_desired_heading_degrees(mech))
      base++;

  if (!(fabsf(speed) > 0.0F))
    return base + 0;
  if (speed > 2.0F * maxspeed / 3.0F + 0.1F)
    return 2;
  return base + 1;
}

int mech_target_movement_modifier(Mech *mech, Mech *target, float range) {
  float target_speed = 0.0;
  int returnValue = 0;
  float m = 1.0;
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(target));
  Mech *swarmTarget;

  if (mech_is_aerospace_unit(target)) {
    if (mech_is_aerospace_unit(mech))
      m = ACCEL_MOD;
    target_speed =
        hypotf(mech_current_speed(target) / m, mech_vertical_speed(target) / m);
  } else {
    if (mech_is_jumping(target)) {
      target_speed = mech_jump_speed_for_map(target, map);
    } else if (mech_condition_summary(target).swarm_target > 0) {
      if ((swarmTarget = btech_context_get_mech(
               context, mech_condition_summary(target).swarm_target))) {
        if (mech_is_jumping(swarmTarget))
          target_speed = mech_jump_speed_for_map(swarmTarget, map);
        else
          target_speed = fabsf(mech_current_speed(swarmTarget));
      }
    } else {
      target_speed = fabsf(mech_current_speed(target));
    }
  }

  if (mech_infantry_technology_flags(target) & CS_PURIFIER_STEALTH_TECH) {
    if (target_speed <= 0.0F) {
      /* Mech moved 0-2 hexes */
      returnValue = 3;
    } else if (target_speed <= MP1) {
      /* Mech moved 3-4 hexes */
      returnValue = 2;
    } else if (target_speed <= MP2) {
      /* Mech moved 5-6 hexes */
      returnValue = 1;
    } else {
      returnValue = 0;
    }
  } else {
    if (target_speed <= MP2) {
      /* Mech moved 0-2 hexes */
      returnValue = 0;
    } else if (target_speed <= MP4) {
      /* Mech moved 3-4 hexes */
      returnValue = 1;
    } else if (target_speed <= MP6) {
      /* Mech moved 5-6 hexes */
      returnValue = 2;
    } else if (target_speed <= MP9) {
      /* Mech moved 7-9 hexes */
      returnValue = 3;
    } else {
      /* Moving more than 9 hexes */
      if (btech_context_uses_extended_movement_modifiers(context))
        returnValue = 4 + (int)((target_speed - 10.0F * MP1) / MP4);
      else
        returnValue = 4;
    }
  }

  if (mech_is_immobile(target))
    returnValue += -4;

  if (mech_condition_summary(target).fallen &&
      ((mech_class(target) == CLASS_MECH) || (mech_class(target) == CLASS_MW)))
    returnValue += (range <= 1.0F) ? -2 : 1;

  if (mech_is_jumping(target))
    returnValue++;

  return (returnValue);
}
