/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "environment_damage_api.h"
#include "legacy_macros.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_stagger.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "section_types.h"

static bool mech_fall_is_in_water(Mech *mech) {
  return battle_terrain_is_water(mech_real_terrain_get(mech)) &&
         mech_position_z(mech) < 0;
}

static int mech_fall_movement_mode_delay(const Mech *mech) {
  return mech_class(mech) == CLASS_BSUIT || mech_class(mech) == CLASS_MW
             ? TURN / 2
             : TURN;
}

void mech_fall(Mech *mech, int levels, int seemsg) {
  int roll, spread, i, hitloc, hitGroup = 0;
  int isrear = 0, damage, iscritical = 0;
  int heading_offset = 0;
  BattleMap *map;
  BtechContext *context = mech_context(mech);

  /* get rid of our swarmers */
  if (CountSwarmers(mech))
    StopBSuitSwarmers(btech_context_find_object(context, mech_map_dbref(mech)),
                      mech, 0);

  /* Clear stagger damage if we use new stagger*/
  if (btech_context_stagger_mode(mech_context(mech)))
    mech_stagger_damage_clear(mech);

  /* damage pilot */
  mech_cocoon_integrity_set(mech, 0);

  /* Rule Reference: BMR Revised, Page 16 ( Fall = Bruise if Pilot roll fails)
   */
  /* Rule Reference: Total Warfare, Page 41 ( Fall = Bruise if Pilot roll fails)
   */

  if (!mech_condition_summary(mech).combat_safe) {
    if (mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_MW ||
        seemsg)
      mech_notify(mech, MECHPILOT,
                  "You try to avoid taking personal damage in the fall.");
    else
      mech_notify(mech, MECHPILOT, "You try to avoid taking personal damage.");
    if (!MadePilotSkillRoll(mech, levels)) {
      if (mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_MW ||
          seemsg)
        mech_notify(mech, MECHPILOT, "You take personal injury from the fall!");
      else
        mech_notify(mech, MECHPILOT, "You take personal injury!");
      headhitmwdamage(mech, mech, 1);
    }
  }

  mech_movement_stop(mech);
  if (mech_is_jumping(mech)) {
    mech_jump_abort(mech);
    mech_event_cancel(mech, EVENT_JUMP);
    mech_event_schedule(mech, EVENT_JUMPSTABIL, mech_stabilizing_event,
                        JUMP_TO_HIT_RECYCLE, 0);
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_event_count(mech, EVENT_MOVEMODE))
    mech_event_cancel(mech, EVENT_MOVEMODE);
  if (mech_condition_summary(mech).sprinting)
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event,
                        mech_fall_movement_mode_delay(mech),
                        MODE_SPRINT | MODE_OFF);
  if (mech_condition_summary(mech).evading)
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event,
                        mech_fall_movement_mode_delay(mech),
                        MODE_EVADE | MODE_OFF);
#endif
  if (mech_movement_type(mech) == MOVE_VTOL ||
      mech_movement_type(mech) == MOVE_FLY) {
    mech_vertical_speed_set(mech, 0.0F);
    mech_jump_destination_y_set(mech, 0);
    mech_motion_vector_reset(mech);
    mech_landed_set(mech, true);
    if (!mech_condition_summary(mech).combat_safe) {
      if (mech_movement_type(mech) == MOVE_VTOL)
        mech_notify(mech, MECHALL, "Your rotor has been destroyed!");
      mech_fallen_set(mech, true);
    }
    mech_event_cancel(mech, EVENT_MOVE);
  } else
    mech_maybe_move(mech);
  if (mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_MW)
    mech_make_fall(mech);

  if (seemsg)
    mech_los_broadcast(mech, "falls down!");
  mech_drop_surface_set(mech, true);
  mech_position_real_z_sync(mech);

  roll = btech_random_range(context, 1, 6);
  switch (roll) {
  case 1:
    hitGroup = FRONT;
    break;
  case 2:
    heading_offset = 60;
    hitGroup = RIGHTSIDE;
    break;
  case 3:
    heading_offset = 120;
    hitGroup = RIGHTSIDE;
    break;
  case 4:
    heading_offset = 180;
    hitGroup = BACK;
    break;
  case 5:
    heading_offset = 240;
    hitGroup = LEFTSIDE;
    break;
  case 6:
    heading_offset = 300;
    hitGroup = LEFTSIDE;
    break;
  }
  if (hitGroup == BACK)
    isrear = 1;
  mech_fall_heading_apply(mech, heading_offset);
  if (!mech_fall_is_in_water(mech) &&
      mech_real_terrain_get(mech) != BATTLE_TERRAIN_HIGH_WATER)
#ifndef REALWEIGHT_DAMAGE
    damage = (levels * (mech_tonnage(mech) + 5)) / 10;
#else
    damage = (levels * (mech_real_tonnage(mech) + 5)) / 10;
#endif /* REALWEIGHT_DAMAGE */
  else
#ifndef REALWEIGHT_DAMAGE
    damage = (levels * (mech_tonnage(mech) + 5)) / 20;
#else
    damage = (levels * (mech_real_tonnage(mech) + 5)) / 20;
#endif /* REALWEIGHT_DAMAGE */
  if (mech_is_under_special_conditions(mech))
    if ((map = btech_context_find_object(context, mech_map_dbref(mech))))
      if (battle_map_uses_special_rules(map))
        damage = damage * MIN(100, battle_map_gravity(map)) / 100;

  if (mech_class(mech) == CLASS_MW)
    damage *= 40;

  spread = damage / 5;

  if (!mech_condition_summary(mech).combat_safe) {
    for (i = 0; i < spread; i++) {
      hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
      DamageMech(mech, mech, 0, -1, hitloc, isrear, iscritical, 5, -1, -1, 0,
                 -1, 0, 0);
      mech_flood(mech);
      mech_inferno_extinguish_in_water(mech);
    }
    if (damage % 5) {
      hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
      DamageMech(mech, mech, 0, -1, hitloc, isrear, iscritical, (damage % 5),
                 -1, -1, 0, -1, 0, 0);
      mech_flood(mech);
      mech_inferno_extinguish_in_water(mech);
    }
  }
  mine_field_trigger(mech, MINE_FALL);
  MarkForLOSUpdate(mech);
}
