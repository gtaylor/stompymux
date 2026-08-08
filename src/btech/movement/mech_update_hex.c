/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_update_api.h"

#include <math.h>
#include <stdlib.h>

#include "mux/network/mux_event.h"

#include "btech/context.h"
#include "btechstats_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_fire_api.h"
#include "mech_hex_transition_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/support/formatting.h"
#include "section_types.h"

static int mech_hex_maximum_int(int first, int second) {
  return first > second ? first : second;
}

/*
 * Check to see what happens to the unit now that its entered a new hex
 */
void mech_hex_entry_resolve(Mech *mech, BattleMap *mech_map, float deltax,
                            float deltay, int last_z) {
  int elevation, lastelevation;
  int oldterrain;
  int ot, le, done = 0, tt, avoidbth;
  int isunder = 0;
  float f;
  BtechContext *context = mech_context(mech);
  bool skid_cliff = btech_context_uses_skid_cliff_rules(context);
  bool roll_on_backwalk = btech_context_uses_roll_on_backwalk(context);
  bool new_terrain = btech_context_uses_new_terrain_rules(context);
  bool advanced_vehicle_fire =
      btech_context_uses_advanced_vehicle_fire(context);

  /* Recording the old elevation and terrain */
  /*! \todo {Wasn't lastelevation passed as an argument 'last_z' ?} */
  ot = oldterrain = map_terrain_get(mech_map, mech_position_previous_x(mech),
                                    mech_position_previous_y(mech));

  if ((mech_movement_type(mech) == MOVE_HOVER) &&
      (oldterrain == BATTLE_TERRAIN_WATER || oldterrain == BATTLE_TERRAIN_ICE ||
       ((oldterrain == BATTLE_TERRAIN_BRIDGE) && (last_z == 0)))) {

    le = lastelevation = elevation = 0;

  } else {

    le = lastelevation =
        battle_map_hex_elevation(mech_map, mech_position_previous_x(mech),
                                 mech_position_previous_y(mech));
    elevation = mech_position_surface_elevation(mech);

    if (mech_movement_type(mech) == MOVE_HOVER && elevation < 0)
      elevation = 0;

    if (ot == BATTLE_TERRAIN_ICE && mech_position_z(mech) >= 0) {
      le = lastelevation = 0;
    }

    /*	if(mech_position_z(mech) < le)
                    le = mech_position_z(mech);
    */
  }

  switch (mech_movement_type(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD: {
    HexTransitionResult result = mech_hex_transition_resolve(
        &(HexMechTransitionInput){.mech = mech,
                                  .map = mech_map,
                                  .delta_x = deltax,
                                  .delta_y = deltay,
                                  .elevation = elevation,
                                  .last_elevation = lastelevation,
                                  .old_terrain = oldterrain,
                                  .old_terrain_code = ot,
                                  .old_elevation_code = le});
    if (result.stop)
      return;
    done = result.done;
    break;
  }
  case MOVE_TRACK:

    if (collision_check(mech, WALK_WALL, lastelevation, oldterrain)) {
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

        if (!skid_cliff) {
          mech_notify(mech, MECHALL, "You smash into a cliff!");
          mech_los_broadcast(mech, "crashes to a cliff!");
          mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4), 0);
        } else {
          mech_notify(mech, MECHALL, "You skid to a violent halt!");
          mech_los_broadcast(mech, "goes into a skid!");
          mech_fall(mech, 0, 0);
        }
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      mech_notify(mech, MECHALL, "You notice a large drop in front of you");
      avoidbth = skid_cliff
                     ? mech_skid_modifier(fabs(mech_current_speed(mech)) / MP1)
                     : ((fabs((mech_current_speed(mech)) + MP1) / MP1) / 3);
      if (mech_pilot_dbref(mech) == -1 ||
          (!mech_condition_summary(mech).auto_fall &&
           MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {
        mech_notify(mech, MECHALL, "You manage to stop before falling off.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid falling off a cliff!");
      } else {
        mech_notify(mech, MECHALL,
                    "You drive off the cliff and fall to the ground below.");
        mech_los_broadcast(mech,
                           "drives off a cliff and falls to the ground below.");
        mech_fall(mech, lastelevation - elevation, 0);
        mech_domino_resolve(mech, MECH_DOMINO_FALL);

        if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER &&
            !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH)) {

          mech_notify(
              mech, MECHALL,
              "You drive into the water and your vehicle becomes inoperable.");
          mech_destroy(mech, mech, 0, KILL_TYPE_FLOOD);
        }

        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;

    } else if (roll_on_backwalk && (mech_current_speed(mech) < 0) &&
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
      return;
    }

    if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
        (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER ||
         (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
          (lastelevation < (elevation - 1)))) &&
        elevation < 0) {

      mech_notify(mech, MECHALL, "You notice a body of water in front of you");
      if (mech_pilot_dbref(mech) == -1 ||
          MadePilotSkillRoll(
              mech, (int)(fabs((mech_current_speed(mech)) + MP1) / MP1) / 3)) {
        mech_notify(mech, MECHALL, "You manage to stop before falling in.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid driving into the water!");
      } else {
        mech_notify(
            mech, MECHALL,
            "You drive into the water and your vehicle becomes inoperable.");
        mech_destroy(mech, mech, 0, KILL_TYPE_FLOOD);
        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;
    }

    /* New terrain restrictions */
    if (new_terrain) {
      tt = mech_real_terrain_get(mech);
      if ((tt == BATTLE_TERRAIN_HEAVY_FOREST) &&
          fabs(mech_current_speed(mech)) > MP1) {

        mech_notify(mech, MECHALL, "You try to dodge the larger trees..");

        if (mech_pilot_dbref(mech) == -1 ||
            MadePilotSkillRoll(
                mech, (int)(fabs(mech_current_speed(mech)) / MP1 / 6))) {

          mech_notify(mech, MECHALL, "You manage to dodge 'em!");
        } else {
          mech_notify(mech, MECHALL,
                      "You swerve, but not enough! This'll hurt!");
          mech_los_broadcast(mech, "cruises headlong at a tree!");
          f = fabs(mech_current_speed(mech));
          mech_current_speed_scale(mech, 0.5F);
          mech_fall(mech, mech_hex_maximum_int(1, (int)sqrt(f / MP1 / 2)), 0);
        }
      }
    }

    /* Slow them if they made an elevation change */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (le > 0) {
      deltax = (le == 1) ? MP2 : MP3;
      mech_current_speed_reduce_toward_zero(mech, deltax);
    }
    break;

  case MOVE_WHEEL:

    /* Cliff ! */
    if (collision_check(mech, WALK_WALL, lastelevation, oldterrain)) {

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

        if (!skid_cliff) {
          mech_notify(mech, MECHALL, "You smash into a cliff!");
          mech_los_broadcast(mech, "crashes to a cliff!");
          mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4), 0);
        } else {
          mech_notify(mech, MECHALL, "You skid to a violent halt!");
          mech_los_broadcast(mech, "skids to a halt!");
          mech_fall(mech, 0, 0);
        }
      }

      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      mech_notify(mech, MECHALL, "You notice a large drop in front of you");
      avoidbth = skid_cliff
                     ? mech_skid_modifier(fabs(mech_current_speed(mech)) / MP1)
                     : ((fabs((mech_current_speed(mech)) + MP1) / MP1) / 3);

      if (mech_pilot_dbref(mech) == -1 ||
          (!mech_condition_summary(mech).auto_fall &&
           MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {

        mech_notify(mech, MECHALL, "You manage to stop before falling off.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid driving off a cliff!");
      } else {
        mech_notify(mech, MECHALL,
                    "You drive off the cliff and fall to the ground below.");
        mech_los_broadcast(mech,
                           "drives off a cliff and falls to the ground below.");
        mech_fall(mech, lastelevation - elevation, 0);
        mech_domino_resolve(mech, MECH_DOMINO_FALL);

        if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER &&
            !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH)) {

          mech_notify(
              mech, MECHALL,
              "You drive into the water and your vehicle becomes inoperable.");
          mech_destroy(mech, mech, 0, KILL_TYPE_FLOOD);
        }

        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;

    } else if (roll_on_backwalk && (mech_current_speed(mech) < 0) &&
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
      return;
    }

    if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
        (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER ||
         (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
          (lastelevation < (elevation - 1)))) &&
        elevation < 0) {

      mech_notify(mech, MECHALL, "You notice a body of water in front of you");

      if (mech_pilot_dbref(mech) == -1 ||
          MadePilotSkillRoll(
              mech, (int)(fabs((mech_current_speed(mech)) + MP1) / MP1) / 3)) {

        mech_notify(mech, MECHALL, "You manage to stop before falling in.");
        mech_los_broadcast(mech, "stops suddenly to driving into the water!");
      } else {
        mech_notify(
            mech, MECHALL,
            "You drive into the water and your vehicle becomes inoperable.");
        mech_destroy(mech, mech, 0, KILL_TYPE_FLOOD);
        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;
    }

    /* New terrain restrictions */
    if (new_terrain) {
      tt = mech_real_terrain_get(mech);
      if ((tt == BATTLE_TERRAIN_HEAVY_FOREST ||
           tt == BATTLE_TERRAIN_LIGHT_FOREST) &&
          fabs(mech_current_speed(mech)) > MP1) {

        mech_notify(mech, MECHALL, "You try to dodge the larger trees..");
        if (mech_pilot_dbref(mech) == -1 ||
            MadePilotSkillRoll(
                mech, (tt == BATTLE_TERRAIN_HEAVY_FOREST ? 3 : 0) +
                          (fabs(mech_current_speed(mech)) / MP1 / 6))) {

          mech_notify(mech, MECHALL, "You manage to dodge 'em!");

        } else {
          mech_notify(mech, MECHALL,
                      "You swerve, but not enough! This'll hurt!");
          mech_los_broadcast(mech, "cruises headlong at a tree!");
          f = fabs(mech_current_speed(mech));
          mech_current_speed_scale(mech, 0.5F);
          mech_fall(mech, mech_hex_maximum_int(1, (int)sqrt(f / MP1 / 2)), 0);
        }

      } else if ((tt == BATTLE_TERRAIN_ROUGH) &&
                 fabs(mech_current_speed(mech)) > MP1) {
        mech_notify(mech, MECHALL, "You try to avoid the rocks..");
        if (mech_pilot_dbref(mech) == -1 ||
            MadePilotSkillRoll(
                mech, (int)(fabs(mech_current_speed(mech)) / MP1 / 6))) {
          mech_notify(mech, MECHALL, "You manage to dodge 'em!");
        } else {
          mech_notify(mech, MECHALL,
                      "You swerve, but not enough! This'll hurt!");
          mech_los_broadcast(mech, "cruises headlong at a rock!");
          f = fabs(mech_current_speed(mech));
          mech_current_speed_scale(mech, 0.5F);
          mech_fall(mech, mech_hex_maximum_int(1, (int)sqrt(f / MP1 / 2)), 0);
        }
      }
    }

    /* Slow them down if they change elevations */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (le > 0) {
      deltax = (le == 1) ? MP2 : MP3;
      mech_current_speed_reduce_toward_zero(mech, deltax);
    }
    break;

  case MOVE_HULL:
  case MOVE_FOIL:
  case MOVE_SUB:

    if ((mech_real_terrain_get(mech) != BATTLE_TERRAIN_WATER &&
         mech_real_terrain_get(mech) != BATTLE_TERRAIN_BRIDGE) ||
        abs(mech_position_elevation(mech)) <
            (abs(mech_position_z(mech)) +
             (mech_movement_type(mech) == MOVE_FOIL ? -1 : 0))) {
      /* Run aground */
      mech_position_elevation_set(mech, le);
      mech_position_terrain_set(mech, ot);
      mech_notify(mech, MECHALL, "You attempt to get too close with ground!");
      if (mech_pilot_dbref(mech) == -1 ||
          MadePilotSkillRoll(
              mech, (int)(fabs((mech_current_speed(mech)) + MP1) / MP1) / 3)) {
        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid running aground!");
        mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      } else {
        mech_notify(mech, MECHALL, "You smash into the ground!");
        mech_los_broadcast(mech, "smashes aground!");
        mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4), 0);
      }
      mech_movement_stop(mech);
      mech_vertical_speed_set(mech, 0.0F);
      return;
    }
    if (elevation > 0)
      elevation = 0;
    break;

  case MOVE_HOVER:

    if (collision_check(mech, WALK_WALL, lastelevation, oldterrain)) {
      mech_position_elevation_set(mech, le);
      mech_position_terrain_set(mech, ot);
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

        if (!skid_cliff) {
          mech_notify(mech, MECHALL, "You smash into a cliff!");
          mech_los_broadcast(mech, "smashes into a cliff!");
          mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4), 0);
        } else {
          mech_notify(mech, MECHALL, "You skid to a violent halt!");
          mech_los_broadcast(mech, "skids to a halt!");
          mech_fall(mech, 0, 0);
        }
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      mech_notify(mech, MECHALL, "You notice a large drop in front of you");

      avoidbth = skid_cliff
                     ? mech_skid_modifier(fabs(mech_current_speed(mech)) / MP1)
                     : ((fabs((mech_current_speed(mech)) + MP1) / MP1) / 3);

      if (mech_pilot_dbref(mech) == -1 ||
          (!mech_condition_summary(mech).auto_fall &&
           MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {

        mech_notify(mech, MECHALL, "You manage to stop before falling off.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid falling off a cliff!");

      } else {

        mech_notify(mech, MECHALL,
                    "You drive off the cliff and fall to the ground below.");
        mech_los_broadcast(mech,
                           "drives off a cliff and falls to the ground below.");
        mech_fall(mech, lastelevation - elevation, 0);
        return;
      }

      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;

    } else if (collision_check(mech, HIT_UNDER_BRIDGE, lastelevation,
                               oldterrain)) {

      mech_notify(mech, MECHALL,
                  "You notice the underside of the bridge in front of you!");

      if (mech_pilot_dbref(mech) == -1 ||
          (!skid_cliff &&
           MadePilotSkillRoll(
               mech,
               (int)(fabs((mech_current_speed(mech)) + MP1) / MP1) / 3)) ||
          (skid_cliff &&
           MadePilotSkillRoll(
               mech,
               mech_skid_modifier(fabs(mech_current_speed(mech)) / MP1)))) {

        mech_notify(mech, MECHALL,
                    "You manage to stop before slamming into the bridge.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid slamming into the bridge!");
      } else {
        mech_notify(mech, MECHALL,
                    "You drive right into the underside of the bridge.");
        mech_los_broadcast(mech,
                           "drives right into the underside of the bridge.");
        mech_fall(mech, 1, 0);
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_movement_stop(mech);
      return;

    } else if (roll_on_backwalk && (mech_current_speed(mech) < 0) &&
               (collision_check(mech, WALK_BACK, lastelevation, oldterrain)) &&
               !isunder) {

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
      return;
    }

    tt = mech_real_terrain_get(mech);
    if ((tt == BATTLE_TERRAIN_HEAVY_FOREST ||
         tt == BATTLE_TERRAIN_LIGHT_FOREST) &&
        fabs(mech_current_speed(mech)) > MP1) {
      mech_notify(mech, MECHALL, "You try to dodge the larger trees..");

      if (mech_pilot_dbref(mech) == -1 ||
          MadePilotSkillRoll(mech,
                             (tt == BATTLE_TERRAIN_HEAVY_FOREST ? 3 : 0) +
                                 (fabs(mech_current_speed(mech)) / MP1 / 6))) {

        mech_notify(mech, MECHALL, "You manage to dodge 'em!");

      } else {
        mech_notify(mech, MECHALL, "You swerve, but not enough! This'll hurt!");
        mech_los_broadcast(mech, "cruises headlong at a tree!");
        f = fabs(mech_current_speed(mech));
        mech_current_speed_scale(mech, 0.5F);
        mech_fall(mech, mech_hex_maximum_int(1, (int)sqrt(f / MP1 / 2)), 0);
      }
    }

    /* Slow the unit down if its made an elevation change */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (le > 0) {
      deltax = (le == 1) ? MP2 : MP3;
      mech_current_speed_reduce_toward_zero(mech, deltax);
    }
    break;

  case MOVE_VTOL:
  case MOVE_FLY:

    if ((mech_is_landed(mech) &&
         mech_real_terrain_get(mech) != BATTLE_TERRAIN_ROAD &&
         mech_real_terrain_get(mech) != BATTLE_TERRAIN_BRIDGE &&
         mech_real_terrain_get(mech) != BATTLE_TERRAIN_GRASSLAND &&
         mech_real_terrain_get(mech) != BATTLE_TERRAIN_BUILDING) ||
        (battle_terrain_is_forest(mech_real_terrain_get(mech)) &&
         mech_position_z(mech) < (mech_position_surface_elevation(mech) + 2))) {

      mech_notify(mech, MECHALL,
                  "You go where no flying thing has ever gone before..");
      if (mech_has_active_pilot(mech) && MadePilotSkillRoll(mech, 5)) {
        mech_notify(mech, MECHALL, "You stop in time!");
        mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      } else {
        mech_notify(mech, MECHALL, "Eww.. You've a bad feeling about this.");
        mech_los_broadcast(mech, "crashes!");
        mech_fall(mech, 1, 0);
      }
      mech_movement_stop(mech);
      return;

    } else if (mech_is_landed(mech) && roll_on_backwalk &&
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
        mech_los_broadcast(
            mech,
            tprintf("%s", (elevation > lastelevation
                               ? "falls on its back walking up an incline."
                               : "falls off the back of a small incline.")));
        mech_fall(mech, (abs(lastelevation - elevation) + 1000), 1);
        mech_movement_stop(mech);
        if (elevation > lastelevation) {
          mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        }
      }
      return;
    }

    if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER)
      return;

    if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_LIGHT_FOREST ||
        mech_real_terrain_get(mech) == BATTLE_TERRAIN_HEAVY_FOREST)
      elevation = mech_position_surface_elevation(mech) + 2;
    else
      elevation = mech_position_surface_elevation(mech);

    if (collision_check(mech, JUMP, 0, 0)) {
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      mech_notify(mech, MECHALL,
                  "You attempt to fly over elevation that is too high!");

      if (mech_pilot_dbref(mech) == -1 ||
          (MadePilotSkillRoll(mech,
                              (int)(mech_position_real_z(mech) / ZSCALE / 3)) &&
           (ot == BATTLE_TERRAIN_GRASSLAND || ot == BATTLE_TERRAIN_ROAD ||
            ot == BATTLE_TERRAIN_BUILDING))) {

        mech_notify(mech, MECHALL, "You land safely.");
        mech_landed_set(mech, true);
        mech_current_speed_set(mech, 0.0F);
        mech_vertical_speed_set(mech, 0.0F);

      } else {
        mech_notify(mech, MECHALL,
                    "You crash into the obstacle and fall from the sky!");
        mech_los_broadcast(mech,
                           "crashes into an obstacle and falls from the sky!");
        mech_fall(mech, mech_drop_height_above_surface(mech) + 1, 0);
        mech_domino_resolve(mech, MECH_DOMINO_FALL);
      }
    }
    break;
  case MOVE_NONE:
    break;
  }

  if (!done) {
    mine_field_trigger(mech, MINE_STEP);
    if (advanced_vehicle_fire && (mech_class(mech) == CLASS_VEH_GROUND) &&
        (mech_position_terrain(mech) == BATTLE_TERRAIN_FIRE))
      vehicle_fire_check(mech, 1);
  }
  MarkForLOSUpdate(mech);
}
