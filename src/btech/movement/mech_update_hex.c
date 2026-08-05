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

#include "mech_position_api.h"

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

  /* Recording the old elevation and terrain */
  /*! \todo {Wasn't lastelevation passed as an argument 'last_z' ?} */
  ot = oldterrain = map_terrain_get(mech_map, MechLastX(mech), MechLastY(mech));

  if ((MechMove(mech) == MOVE_HOVER) &&
      (oldterrain == WATER || oldterrain == ICE ||
       ((oldterrain == BRIDGE) && (last_z == 0)))) {

    le = lastelevation = elevation = 0;

  } else {

    le = lastelevation = Elevation(mech_map, MechLastX(mech), MechLastY(mech));
    elevation = MechElevation(mech);

    if (MechMove(mech) == MOVE_HOVER && elevation < 0)
      elevation = 0;

    if (ot == ICE && MechZ(mech) >= 0) {
      le = lastelevation = 0;
    }

    /*	if(MechZ(mech) < le)
                    le = MechZ(mech);
    */
  }

  switch (MechMove(mech)) {
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

      if (MechPilot(mech) == -1 ||
          (!mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(
               mech, (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3, 1)) ||
          (mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(
               mech, mech_skid_modifier(fabs(MechSpeed(mech)) / MP1), 1))) {

        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");

      } else {

        if (!mech->xcode.context->configuration->btech_skidcliff) {
          mech_notify(mech, MECHALL, "You smash into a cliff!");
          mech_los_broadcast(mech, "crashes to a cliff!");
          mech_fall(mech, (int)(MechSpeed(mech) * MP_PER_KPH / 4), 0);
        } else {
          mech_notify(mech, MECHALL, "You skid to a violent halt!");
          mech_los_broadcast(mech, "goes into a skid!");
          mech_fall(mech, 0, 0);
        }
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      mech_notify(mech, MECHALL, "You notice a large drop in front of you");
      avoidbth = mech->xcode.context->configuration->btech_skidcliff
                     ? mech_skid_modifier(fabs(MechSpeed(mech)) / MP1)
                     : ((fabs((MechSpeed(mech)) + MP1) / MP1) / 3);
      if (MechPilot(mech) == -1 ||
          (!MechAutoFall(mech) && MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {
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

        if (mech_real_terrain_get(mech) == WATER &&
            !(MechSpecials2(mech) & WATERPROOF_TECH)) {

          mech_notify(
              mech, MECHALL,
              "You drive into the water and your vehicle becomes inoperable.");
          DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
        }

        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

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
        mech_los_broadcast(
            mech,
            tprintf("%s", (elevation > lastelevation
                               ? "falls on its back walking up an incline."
                               : "falls off the back of a small incline.")));
        mech_fall(mech, abs(lastelevation - elevation), 1);
        MechDesiredSpeed(mech) = 0;
        MechSpeed(mech) = 0;
        if (elevation > lastelevation) {
          mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        }
      }
      return;
    }

    if (!(MechSpecials2(mech) & WATERPROOF_TECH) &&
        (mech_real_terrain_get(mech) == WATER ||
         (mech_real_terrain_get(mech) == BRIDGE &&
          (lastelevation < (elevation - 1)))) &&
        elevation < 0) {

      mech_notify(mech, MECHALL, "You notice a body of water in front of you");
      if (MechPilot(mech) == -1 ||
          MadePilotSkillRoll(mech,
                             (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3)) {
        mech_notify(mech, MECHALL, "You manage to stop before falling in.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid driving into the water!");
      } else {
        mech_notify(
            mech, MECHALL,
            "You drive into the water and your vehicle becomes inoperable.");
        DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;
    }

    /* New terrain restrictions */
    if (mech->xcode.context->configuration->btech_newterrain) {
      tt = mech_real_terrain_get(mech);
      if ((tt == HEAVY_FOREST) && fabs(MechSpeed(mech)) > MP1) {

#if 0
				mech_notify(mech, MECHALL, "You cruise at a bunch of trees!");
#endif
        mech_notify(mech, MECHALL, "You try to dodge the larger trees..");

        if (MechPilot(mech) == -1 ||
            MadePilotSkillRoll(mech, (int)(fabs(MechSpeed(mech)) / MP1 / 6))) {

          mech_notify(mech, MECHALL, "You manage to dodge 'em!");
        } else {
          mech_notify(mech, MECHALL,
                      "You swerve, but not enough! This'll hurt!");
          mech_los_broadcast(mech, "cruises headlong at a tree!");
          f = fabs(MechSpeed(mech));
          MechSpeed(mech) = MechSpeed(mech) / 2.0;
          mech_fall(mech, MAX(1, (int)sqrt(f / MP1 / 2)), 0);
        }
      }
    }

    /* Slow them if they made an elevation change */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (le > 0) {
      deltax = (le == 1) ? MP2 : MP3;
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
    break;

  case MOVE_WHEEL:

    /* Cliff ! */
    if (collision_check(mech, WALK_WALL, lastelevation, oldterrain)) {

      mech_notify(mech, MECHALL,
                  "You attempt to climb a hill too steep for you.");

      if (MechPilot(mech) == -1 ||
          (!mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(
               mech, (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3, 1)) ||
          (mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(
               mech, mech_skid_modifier(fabs(MechSpeed(mech)) / MP1), 1))) {

        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");

      } else {

        if (!mech->xcode.context->configuration->btech_skidcliff) {
          mech_notify(mech, MECHALL, "You smash into a cliff!");
          mech_los_broadcast(mech, "crashes to a cliff!");
          mech_fall(mech, (int)(MechSpeed(mech) * MP_PER_KPH / 4), 0);
        } else {
          mech_notify(mech, MECHALL, "You skid to a violent halt!");
          mech_los_broadcast(mech, "skids to a halt!");
          mech_fall(mech, 0, 0);
        }
      }

      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      mech_notify(mech, MECHALL, "You notice a large drop in front of you");
      avoidbth = mech->xcode.context->configuration->btech_skidcliff
                     ? mech_skid_modifier(fabs(MechSpeed(mech)) / MP1)
                     : ((fabs((MechSpeed(mech)) + MP1) / MP1) / 3);

      if (MechPilot(mech) == -1 ||
          (!MechAutoFall(mech) && MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {

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

        if (mech_real_terrain_get(mech) == WATER &&
            !(MechSpecials2(mech) & WATERPROOF_TECH)) {

          mech_notify(
              mech, MECHALL,
              "You drive into the water and your vehicle becomes inoperable.");
          DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
        }

        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

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
        mech_los_broadcast(
            mech,
            tprintf("%s", (elevation > lastelevation
                               ? "falls on its back walking up an incline."
                               : "falls off the back of a small incline.")));
        mech_fall(mech, abs(lastelevation - elevation), 1);
        MechDesiredSpeed(mech) = 0;
        MechSpeed(mech) = 0;
        if (elevation > lastelevation) {
          mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        }
      }
      return;
    }

    if (!(MechSpecials2(mech) & WATERPROOF_TECH) &&
        (mech_real_terrain_get(mech) == WATER ||
         (mech_real_terrain_get(mech) == BRIDGE &&
          (lastelevation < (elevation - 1)))) &&
        elevation < 0) {

      mech_notify(mech, MECHALL, "You notice a body of water in front of you");

      if (MechPilot(mech) == -1 ||
          MadePilotSkillRoll(mech,
                             (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3)) {

        mech_notify(mech, MECHALL, "You manage to stop before falling in.");
        mech_los_broadcast(mech, "stops suddenly to driving into the water!");
      } else {
        mech_notify(
            mech, MECHALL,
            "You drive into the water and your vehicle becomes inoperable.");
        DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
        return;
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;
    }

    /* New terrain restrictions */
    if (mech->xcode.context->configuration->btech_newterrain) {
      tt = mech_real_terrain_get(mech);
      if ((tt == HEAVY_FOREST || tt == LIGHT_FOREST) &&
          fabs(MechSpeed(mech)) > MP1) {

#if 0
				mech_notify(mech, MECHALL, "You cruise at a bunch of trees!");
#endif
        mech_notify(mech, MECHALL, "You try to dodge the larger trees..");
        if (MechPilot(mech) == -1 ||
            MadePilotSkillRoll(mech, (tt == HEAVY_FOREST ? 3 : 0) +
                                         (fabs(MechSpeed(mech)) / MP1 / 6))) {

          mech_notify(mech, MECHALL, "You manage to dodge 'em!");

        } else {
          mech_notify(mech, MECHALL,
                      "You swerve, but not enough! This'll hurt!");
          mech_los_broadcast(mech, "cruises headlong at a tree!");
          f = fabs(MechSpeed(mech));
          MechSpeed(mech) = MechSpeed(mech) / 2.0;
          mech_fall(mech, MAX(1, (int)sqrt(f / MP1 / 2)), 0);
        }

      } else if ((tt == ROUGH) && fabs(MechSpeed(mech)) > MP1) {
#if 0
				mech_notify(mech, MECHALL,
							"You cruise at some rough terrain!");
#endif
        mech_notify(mech, MECHALL, "You try to avoid the rocks..");
        if (MechPilot(mech) == -1 ||
            MadePilotSkillRoll(mech, (int)(fabs(MechSpeed(mech)) / MP1 / 6))) {
          mech_notify(mech, MECHALL, "You manage to dodge 'em!");
        } else {
          mech_notify(mech, MECHALL,
                      "You swerve, but not enough! This'll hurt!");
          mech_los_broadcast(mech, "cruises headlong at a rock!");
          f = fabs(MechSpeed(mech));
          MechSpeed(mech) = MechSpeed(mech) / 2.0;
          mech_fall(mech, MAX(1, (int)sqrt(f / MP1 / 2)), 0);
        }
      }
    }

    /* Slow them down if they change elevations */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (le > 0) {
      deltax = (le == 1) ? MP2 : MP3;
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
    break;

  case MOVE_HULL:
  case MOVE_FOIL:
  case MOVE_SUB:

    if ((mech_real_terrain_get(mech) != WATER &&
         mech_real_terrain_get(mech) != BRIDGE) ||
        abs(MechElev(mech)) <
            (abs(MechZ(mech)) + (MechMove(mech) == MOVE_FOIL ? -1 : 0))) {
      /* Run aground */
      MechElev(mech) = le;
      MechTerrain(mech) = ot;
      mech_notify(mech, MECHALL, "You attempt to get too close with ground!");
      if (MechPilot(mech) == -1 ||
          MadePilotSkillRoll(mech,
                             (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3)) {
        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid running aground!");
        mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      } else {
        mech_notify(mech, MECHALL, "You smash into the ground!");
        mech_los_broadcast(mech, "smashes aground!");
        mech_fall(mech, (int)(MechSpeed(mech) * MP_PER_KPH / 4), 0);
      }
      MechSpeed(mech) = 0;
      MechDesiredSpeed(mech) = 0;
      MechVerticalSpeed(mech) = 0;
      return;
    }
    if (elevation > 0)
      elevation = 0;
    break;

  case MOVE_HOVER:

    if (collision_check(mech, WALK_WALL, lastelevation, oldterrain)) {
      MechElev(mech) = le;
      MechTerrain(mech) = ot;
      mech_notify(mech, MECHALL,
                  "You attempt to climb a hill too steep for you.");
      if (MechPilot(mech) == -1 ||
          (!mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(
               mech, (int)(fabs((MechSpeed(mech)) + MP1) / MP1) / 3, 1)) ||
          (mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll_NoXP(
               mech, mech_skid_modifier(fabs(MechSpeed(mech)) / MP1), 1))) {

        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");

      } else {

        if (!mech->xcode.context->configuration->btech_skidcliff) {
          mech_notify(mech, MECHALL, "You smash into a cliff!");
          mech_los_broadcast(mech, "smashes into a cliff!");
          mech_fall(mech, (int)(MechSpeed(mech) * MP_PER_KPH / 4), 0);
        } else {
          mech_notify(mech, MECHALL, "You skid to a violent halt!");
          mech_los_broadcast(mech, "skids to a halt!");
          mech_fall(mech, 0, 0);
        }
      }
      mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

    } else if (collision_check(mech, WALK_DROP, lastelevation, oldterrain)) {

      mech_notify(mech, MECHALL, "You notice a large drop in front of you");

      avoidbth = mech->xcode.context->configuration->btech_skidcliff
                     ? mech_skid_modifier(fabs(MechSpeed(mech)) / MP1)
                     : ((fabs((MechSpeed(mech)) + MP1) / MP1) / 3);

      if (MechPilot(mech) == -1 ||
          (!MechAutoFall(mech) && MadePilotSkillRoll_NoXP(mech, avoidbth, 1))) {

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
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

    } else if (collision_check(mech, HIT_UNDER_BRIDGE, lastelevation,
                               oldterrain)) {

      mech_notify(mech, MECHALL,
                  "You notice the underside of the bridge in front of you!");

      if (MechPilot(mech) == -1 ||
          (!mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll(mech, (int)(fabs((MechSpeed(mech)) + MP1) / MP1) /
                                        3)) ||
          (mech->xcode.context->configuration->btech_skidcliff &&
           MadePilotSkillRoll(
               mech, mech_skid_modifier(fabs(MechSpeed(mech)) / MP1)))) {

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
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

    } else if (mech->xcode.context->configuration->btech_roll_on_backwalk &&
               (MechSpeed(mech) < 0) &&
               (collision_check(mech, WALK_BACK, lastelevation, oldterrain)) &&
               !isunder) {

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
        mech_los_broadcast(
            mech,
            tprintf("%s", (elevation > lastelevation
                               ? "falls on its back walking up an incline."
                               : "falls off the back of a small incline.")));
        mech_fall(mech, abs(lastelevation - elevation), 1);
        MechDesiredSpeed(mech) = 0;
        MechSpeed(mech) = 0;
        if (elevation > lastelevation) {
          mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        }
      }
      return;
    }

    tt = mech_real_terrain_get(mech);
    if ((tt == HEAVY_FOREST || tt == LIGHT_FOREST) &&
        fabs(MechSpeed(mech)) > MP1) {
#if 0
			mech_notify(mech, MECHALL, "You cruise at a bunch of trees!");
#endif
      mech_notify(mech, MECHALL, "You try to dodge the larger trees..");

      if (MechPilot(mech) == -1 ||
          MadePilotSkillRoll(mech, (tt == HEAVY_FOREST ? 3 : 0) +
                                       (fabs(MechSpeed(mech)) / MP1 / 6))) {

        mech_notify(mech, MECHALL, "You manage to dodge 'em!");

      } else {
        mech_notify(mech, MECHALL, "You swerve, but not enough! This'll hurt!");
        mech_los_broadcast(mech, "cruises headlong at a tree!");
        f = fabs(MechSpeed(mech));
        MechSpeed(mech) = MechSpeed(mech) / 2.0;
        mech_fall(mech, MAX(1, (int)sqrt(f / MP1 / 2)), 0);
      }
    }

    /* Slow the unit down if its made an elevation change */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (le > 0) {
      deltax = (le == 1) ? MP2 : MP3;
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
    break;

  case MOVE_VTOL:
  case MOVE_FLY:

    if ((Landed(mech) && mech_real_terrain_get(mech) != ROAD &&
         mech_real_terrain_get(mech) != BRIDGE &&
         mech_real_terrain_get(mech) != GRASSLAND &&
         mech_real_terrain_get(mech) != BUILDING) ||
        (IsForest(mech_real_terrain_get(mech)) &&
         MechZ(mech) < (MechElevation(mech) + 2))) {

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
      MechDesiredSpeed(mech) = 0;
      MechSpeed(mech) = 0;
      return;

    } else if (Landed(mech) &&
               mech->xcode.context->configuration->btech_roll_on_backwalk &&
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
        mech_los_broadcast(
            mech,
            tprintf("%s", (elevation > lastelevation
                               ? "falls on its back walking up an incline."
                               : "falls off the back of a small incline.")));
        mech_fall(mech, (abs(lastelevation - elevation) + 1000), 1);
        MechDesiredSpeed(mech) = 0;
        MechSpeed(mech) = 0;
        if (elevation > lastelevation) {
          mech_position_rollback(mech, deltax, deltay, lastelevation, ot, le);
        }
      }
      return;
    }

    if (mech_real_terrain_get(mech) == WATER)
      return;

    if (mech_real_terrain_get(mech) == LIGHT_FOREST ||
        mech_real_terrain_get(mech) == HEAVY_FOREST)
      elevation = MechElevation(mech) + 2;
    else
      elevation = MechElevation(mech);

    if (collision_check(mech, JUMP, 0, 0)) {
      MechFX(mech) -= deltax;
      MechFY(mech) -= deltay;
      MechX(mech) = MechLastX(mech);
      MechY(mech) = MechLastY(mech);
      MechZ(mech) = lastelevation;
      MechFZ(mech) = MechZ(mech) * ZSCALE;
      MechElev(mech) = le;
      MechTerrain(mech) = ot;
      mech_notify(mech, MECHALL,
                  "You attempt to fly over elevation that is too high!");

      if (MechPilot(mech) == -1 ||
          (MadePilotSkillRoll(mech, (int)(MechFZ(mech) / ZSCALE / 3)) &&
           (ot == GRASSLAND || ot == ROAD || ot == BUILDING))) {

        mech_notify(mech, MECHALL, "You land safely.");
        MechStatus(mech) |= LANDED;
        MechSpeed(mech) = 0.0;
        MechVerticalSpeed(mech) = 0.0;

      } else {
        mech_notify(mech, MECHALL,
                    "You crash into the obstacle and fall from the sky!");
        mech_los_broadcast(mech,
                           "crashes into an obstacle and falls from the sky!");
        mech_fall(mech, MechsElevation(mech) + 1, 0);
        mech_domino_resolve(mech, MECH_DOMINO_FALL);
      }
    }
    break;
  }

  if (!done) {
    possible_mine_poof(mech, MINE_STEP);
    if (mech->xcode.context->configuration->btech_fasaadvvhlfire &&
        (MechType(mech) == CLASS_VEH_GROUND) && (MechTerrain(mech) == FIRE))
      checkVehicleInFire(mech, 1);
  }
  MarkForLOSUpdate(mech);
}
