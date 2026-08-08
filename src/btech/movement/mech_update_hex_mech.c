/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_hex_transition_api.h"

#include <math.h>
#include <stdlib.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"

HexTransitionResult
mech_hex_transition_resolve(const HexMechTransitionInput *input) {
  Mech *mech = input->mech;
  BattleMap *mech_map = input->map;
  float deltax = input->delta_x;
  float deltay = input->delta_y;
  int elevation = input->elevation;
  int lastelevation = input->last_elevation;
  int oldterrain = input->old_terrain;
  int ot = input->old_terrain_code;
  int le = input->old_elevation_code;
  int ed;
  int avoidbth;
  int done = 0;
  BtechContext *context = mech_context(mech);
  bool skid_cliff = btech_context_uses_skid_cliff_rules(context);
  MechConditionSummary condition = mech_condition_summary(mech);

  switch ((int)mech_movement_type(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD:

    if (mech_is_jumping(mech)) {

      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER)
        return (HexTransitionResult){.stop = true, .done = done};

      /* Did we hit something while jumping */
      if (collision_check(mech, JUMP, 0, 0)) {

        ed = 1 + mech_position_z(mech) -
             battle_map_hex_elevation(mech_map, mech_position_x(mech),
                                      mech_position_y(mech));
        ed = ed > 1 ? ed : 1;
        mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        mech_notify(mech, MECHALL,
                    "[bold]You attempt to jump over elevation that is too "
                    "high![reset]");
        if (mech_has_active_pilot(mech) &&
            MadePilotSkillRoll(mech,
                               (int)mech_position_real_z(mech) / ZSCALE / 3)) {

          mech_notify(mech, MECHALL, "[bold]You land safely.[reset]");
          mech_jump_land(mech);

        } else {

          mech_notify(mech, MECHALL,
                      "[bold]You crash into the obstacle and fall from the "
                      "sky![reset]");
          mech_los_broadcast(
              mech, "crashes into an obstacle and falls from the sky!");
          mech_fall(mech, ed, 0);
          mech_domino_resolve(mech, MECH_DOMINO_FALL);
        }
      }
      return (HexTransitionResult){.stop = true, .done = done};
    }

    /* Walked into a wall silly */
    if (collision_check(mech, WALK_WALL, lastelevation, oldterrain)) {

      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_notify(mech, MECHALL,
                  "You attempt to climb a hill too steep for you.");

      if (mech_pilot_dbref(mech) == -1 ||
          (!skid_cliff &&
           MadePilotSkillRoll_NoXP(
               mech, (int)(fabs((mech_current_speed(mech)) + MP1) / MP1) / 3,
               1)) ||
          (skid_cliff &&
           MadePilotSkillRoll_NoXP(
               mech, mech_skid_modifier(fabs(mech_current_speed(mech)) / MP1),
               1))) {

        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");

      } else {

        mech_notify(mech, MECHALL,
                    "You run headlong into the cliff and fall down!");
        mech_los_broadcast(mech, "runs headlong into a cliff and falls down!");
        if (!skid_cliff)
          mech_fall(mech, (int)(1 + mech_current_speed(mech) * MP_PER_KPH) / 4,
                    0);
        else
          mech_fall(mech, 1, 0);
      }
      mech_movement_stop(mech);
      mech_position_z_set(mech, lastelevation);
      return (HexTransitionResult){.stop = true, .done = done};

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      /* Walked off a cliff ... */
      mech_notify(mech, MECHALL, "You notice a large drop in front of you");
      avoidbth = skid_cliff
                     ? mech_skid_modifier(fabs(mech_current_speed(mech)) / MP1)
                     : ((fabs((mech_current_speed(mech)) + MP1) / MP1) / 3);

      if (mech_pilot_dbref(mech) == -1 ||
          (!condition.auto_fall &&
           MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {

        mech_notify(mech, MECHALL, "You manage to stop before falling off.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid falling off a cliff!");
        mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);

      } else {

        mech_notify(mech, MECHALL,
                    "You run off the cliff and fall to the ground below.");
        mech_los_broadcast(mech,
                           "runs off a cliff and falls to the ground below!");
        mech_fall(mech, lastelevation - elevation, 0);
        mech_movement_stop(mech);
      }
      mech_movement_stop(mech);
      return (HexTransitionResult){.stop = true, .done = done};

    } else if (btech_context_requires_backwalk_rolls(context) &&
               (mech_current_speed(mech) < 0) &&
               (collision_check(mech, WALK_BACK, lastelevation, oldterrain))) {

      mech_printf(mech, MECHALL, "You notice a %s behind you!",
                  (elevation > lastelevation ? "small incline" : "small drop"));

      if (mech_pilot_dbref(mech) == -1 ||
          (MadePilotSkillRoll(mech, collision_check(mech, WALK_BACK,
                                                    lastelevation, oldterrain) -
                                        1))) {

        mech_notify(mech, MECHALL, "You manage to overcome the obstacle.");

      } else {

        mech_printf(mech, MECHALL, "%s",
                    (elevation > lastelevation
                         ? "You stumble on your rear and fall down."
                         : "You fall on your rear off the small incline."));

        /*! \todo {Get rid of this tprintf} */
        mech_los_broadcast(
            mech,
            tprintf("%s", (elevation > lastelevation
                               ? "falls on its back walking up an incline."
                               : "falls off the back of a small incline.")));
        mech_fall(mech, abs(lastelevation - elevation), 1);
        mech_movement_stop(mech);
        if (elevation > lastelevation) {
          mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        }
      }
      return (HexTransitionResult){.stop = true, .done = done};
    }

    /* Slow the unit if its made an elevation change */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (mech_position_z(mech) != elevation)
      le = 0;
    if (le > 0) {
      deltax = (le == 1) ? MP1 : MP2;
      float speed = mech_current_speed(mech);
      if (speed > 0) {
        speed -= deltax;
        mech_current_speed_set(mech, speed < 0 ? 0 : speed);
      } else if (speed < 0) {
        speed += deltax;
        mech_current_speed_set(mech, speed > 0 ? 0 : speed);
      }
    }

    if (mech_class(mech) == CLASS_BSUIT) {

      /* Are they in water, also make sure it affects them */
      if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
          (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER ||
           (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
            (lastelevation < (elevation - 1)))) &&
          elevation < 0) {

        mech_notify(mech, MECHALL,
                    "You notice a body of water in front of you");

        if (mech_pilot_dbref(mech) == -1 ||
            MadePilotSkillRoll(
                mech,
                (int)(fabs((mech_current_speed(mech)) + MP1) / MP1) / 3)) {

          mech_notify(mech, MECHALL, "You manage to stop before falling in.");
          mech_los_broadcast(mech, "stops suddenly to avoid going for a swim!");
        } else {

          mech_notify(mech, MECHALL,
                      "You trip at the edge of the water and plunge in...");
          mech_flood(mech);
          return (HexTransitionResult){.stop = true, .done = done};
        }
        mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        mech_movement_stop(mech);
        return (HexTransitionResult){.stop = true, .done = done};
      }

    } else if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
               ((mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER &&
                 mech_position_z(mech) < 0) ||
                (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
                 mech_position_z(mech) < 0) ||
                (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE &&
                 mech_position_z(mech) < 0) ||
                mech_real_terrain_get(mech) == BATTLE_TERRAIN_HIGH_WATER) &&
               mech_class(mech) != CLASS_MW) {

      int skillmod, dammod;
      float walking_speed = 2.0F * mech_effective_maximum_speed(mech) / 3.0F;
      if (mech_desired_speed(mech) > walking_speed)
        mech_desired_speed_set(mech, walking_speed);
#ifdef BT_MOVEMENT_MODES
      if (condition.sprinting) {
        mech_los_broadcast(mech,
                           "breaks out of its sprint as it enters water!");
        mech_notify(mech, MECHALL,
                    "You lose your sprinting momentum as you "
                    "enter water!");
        if (!mech_event_count(mech, EVENT_MOVEMODE))
          mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event, TURN,
                              MODE_OFF | MODE_SPRINT);
      }
#endif
      if (mech_current_speed(mech) >
          2.0F * mech_effective_maximum_speed(mech) / 3.0F + 0.1F) {
        mech_notify(mech, MECHPILOT,
                    "You struggle to keep control as you run into the water!");
        skillmod = 2;
        dammod = 2;
      } else {
        mech_notify(mech, MECHPILOT,
                    "You use your piloting skill "
                    "to maneuver through the water.");
        skillmod = 0;
        dammod = 0;
      }
      skillmod += (mech_real_terrain_get(mech) == BATTLE_TERRAIN_HIGH_WATER ? -2
                   : mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE
                       ? bridge_w_elevation(mech)
                   : mech_position_elevation(mech) > 3
                       ? 1
                       : (mech_position_elevation(mech) - 2));
      //
      // Stupid Frontiers cheaters. No XP gains here.
      if (!MadePilotSkillRoll_NoXP(mech, skillmod, 0)) {
        mech_notify(mech, MECHALL, "You slip in the water and fall down");
        mech_los_broadcast(mech, "slips in the water and falls down!");
        mech_fall(mech, 1, dammod);
        done = 1;
      }
    }
    break;
  }
  return (HexTransitionResult){.done = done};
}
