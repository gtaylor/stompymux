/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_update_internal.h"

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

  switch (MechMove(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD:

    if (Jumping(mech)) {

      if (mech_real_terrain_get(mech) == WATER)
        return (HexTransitionResult){.stop = true, .done = done};

      /* Did we hit something while jumping */
      if (collision_check(mech, JUMP, 0, 0)) {

        ed = MAX(1, 1 + MechZ(mech) -
                        Elevation(mech_map, MechX(mech), MechY(mech)));
        move_unit_back(mech, deltax, deltay, lastelevation, ot, le);
        mech_notify(mech, MECHALL,
                    "[bold]You attempt to jump over elevation that is too "
                    "high![reset]");
        if (mech_has_active_pilot(mech) &&
            MadePilotSkillRoll(mech, (int)(MechFZ(mech)) / ZSCALE / 3)) {

          mech_notify(mech, MECHALL, "[bold]You land safely.[reset]");
          LandMech(mech);

        } else {

          mech_notify(mech, MECHALL,
                      "[bold]You crash into the obstacle and fall from the "
                      "sky![reset]");
          mech_los_broadcast(
              mech, "crashes into an obstacle and falls from the sky!");
          MechFalls(mech, ed, 0);
          mech_domino_resolve(mech, MECH_DOMINO_FALL);
        }
      }
      return (HexTransitionResult){.stop = true, .done = done};
    }

    /* Walked into a wall silly */
    if (collision_check(mech, WALK_WALL, lastelevation, oldterrain)) {

      move_unit_back(mech, deltax, deltay, lastelevation, ot, le);
      mech_notify(mech, MECHALL,
                  "You attempt to climb a hill too steep for you.");

      if (MechPilot(mech) == -1 ||
          (!mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(
               mech, (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3, 1)) ||
          (mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(mech, SkidMod(fabs(MechSpeed(mech)) / MP1),
                                   1))) {

        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");

      } else {

        mech_notify(mech, MECHALL,
                    "You run headlong into the cliff and fall down!");
        mech_los_broadcast(mech, "runs headlong into a cliff and falls down!");
        if (!mech->xcode.context->configuration->btech_skidcliff)
          MechFalls(mech, (int)(1 + (MechSpeed(mech)) * MP_PER_KPH) / 4, 0);
        else
          MechFalls(mech, 1, 0);
      }
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      MechZ(mech) = lastelevation;
      return (HexTransitionResult){.stop = true, .done = done};

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      /* Walked off a cliff ... */
      mech_notify(mech, MECHALL, "You notice a large drop in front of you");
      avoidbth = mech->xcode.context->configuration->btech_skidcliff
                     ? SkidMod(fabs(MechSpeed(mech)) / MP1)
                     : ((fabs((MechSpeed(mech)) + MP1) / MP1) / 3);

      if (MechPilot(mech) == -1 ||
          (!MechAutoFall(mech) && MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {

        mech_notify(mech, MECHALL, "You manage to stop before falling off.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid falling off a cliff!");
        move_unit_back(mech, deltax, deltay, lastelevation, ot, le);

      } else {

        mech_notify(mech, MECHALL,
                    "You run off the cliff and fall to the ground below.");
        mech_los_broadcast(mech,
                           "runs off a cliff and falls to the ground below!");
        MechFalls(mech, lastelevation - elevation, 0);
        MechDesiredSpeed(mech) = 0;
        MechSpeed(mech) = 0;
      }
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return (HexTransitionResult){.stop = true, .done = done};

    } else if (mech->xcode.context->configuration->btech_roll_on_backwalk &&
               (MechSpeed(mech) < 0) &&
               (collision_check(mech, WALK_BACK, lastelevation, oldterrain))) {

      mech_printf(mech, MECHALL, "You notice a %s behind you!",
                  (elevation > lastelevation ? "small incline" : "small drop"));

      if (MechPilot(mech) == -1 ||
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
        MechFalls(mech, abs(lastelevation - elevation), 1);
        MechDesiredSpeed(mech) = 0;
        MechSpeed(mech) = 0;
        if (elevation > lastelevation) {
          move_unit_back(mech, deltax, deltay, lastelevation, ot, le);
        }
      }
      return (HexTransitionResult){.stop = true, .done = done};
    }

    /* Slow the unit if its made an elevation change */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (MechZ(mech) != elevation)
      le = 0;
    if (le > 0) {
      deltax = (le == 1) ? MP1 : MP2;
      if (MechSpeed(mech) > 0) {
        MechSpeed(mech) -= deltax;
        if (MechSpeed(mech) < 0)
          MechSpeed(mech) = 0;
      } else if (MechSpeed(mech) < 0) {
        MechSpeed(mech) += deltax;
        if (MechSpeed(mech) > 0)
          MechSpeed(mech) = 0;
      }
    }

    if (MechType(mech) == CLASS_BSUIT) {

      /* Are they in water, also make sure it affects them */
      if (!(MechSpecials2(mech) & WATERPROOF_TECH) &&
          (mech_real_terrain_get(mech) == WATER ||
           (mech_real_terrain_get(mech) == BRIDGE &&
            (lastelevation < (elevation - 1)))) &&
          elevation < 0) {

        mech_notify(mech, MECHALL,
                    "You notice a body of water in front of you");

        if (MechPilot(mech) == -1 ||
            MadePilotSkillRoll(
                mech, (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3)) {

          mech_notify(mech, MECHALL, "You manage to stop before falling in.");
          mech_los_broadcast(mech, "stops suddenly to avoid going for a swim!");
        } else {

          mech_notify(mech, MECHALL,
                      "You trip at the edge of the water and plunge in...");
          MechFloods(mech);
          return (HexTransitionResult){.stop = true, .done = done};
        }
        move_unit_back(mech, deltax, deltay, lastelevation, ot, le);
        MechDesiredSpeed(mech) = 0;
        MechSpeed(mech) = 0;
        return (HexTransitionResult){.stop = true, .done = done};
      }

    } else if (!(MechSpecials2(mech) & WATERPROOF_TECH) &&
               ((mech_real_terrain_get(mech) == WATER && MechZ(mech) < 0) ||
                (mech_real_terrain_get(mech) == BRIDGE && MechZ(mech) < 0) ||
                (mech_real_terrain_get(mech) == ICE && MechZ(mech) < 0) ||
                mech_real_terrain_get(mech) == HIGHWATER) &&
               MechType(mech) != CLASS_MW) {

      int skillmod, dammod;
      MechDesiredSpeed(mech) =
          MIN(MechDesiredSpeed(mech), WalkingSpeed(MMaxSpeed(mech)));
#ifdef BT_MOVEMENT_MODES
      if (MechStatus2(mech) & SPRINTING) {
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
      if (IsRunning(MechSpeed(mech), MMaxSpeed(mech))) {
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
      skillmod +=
          (mech_real_terrain_get(mech) == HIGHWATER ? -2
           : mech_real_terrain_get(mech) == BRIDGE  ? bridge_w_elevation(mech)
           : MechElev(mech) > 3                     ? 1
                                                    : (MechElev(mech) - 2));
      //
      // Stupid Frontiers cheaters. No XP gains here.
      if (!MadePilotSkillRoll_NoXP(mech, skillmod, 0)) {
        mech_notify(mech, MECHALL, "You slip in the water and fall down");
        mech_los_broadcast(mech, "slips in the water and falls down!");
        MechFalls(mech, 1, dammod);
        done = 1;
      }
    }
    break;
  }
  return (HexTransitionResult){.done = done};
}
