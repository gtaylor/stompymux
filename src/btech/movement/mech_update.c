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

int fiery_death(Mech *mech) {
  if (MechTerrain(mech) == FIRE) {
    if (is_aero(mech))
      return 0;
    if (MechMove(mech) == MOVE_VTOL)
      return 0;
    if (Destroyed(mech))
      return 0;

    /* Cause various things */
    /* MWs _die_ */
    if (MechType(mech) == CLASS_MW) {
      mech_notify(mech, MECHALL, "You feel a tad bit too warm..");
      mech_notify(mech, MECHALL, "You faint.");
      DestroyMech(mech, mech, 0, KILL_TYPE_HEAT);
      return 1;
    }

    /* Tanks may die */
    return 0; /* Dumb idea */
    heat_effect(NULL, mech, 0, 0);
    if (Destroyed(mech))
      return 1;
  }
  return 0;
}

int bridge_w_elevation(Mech *mech) { return -1; }

void bridge_set_elevation(Mech *mech) {
  if (MechZ(mech) < (MechUpperElevation(mech) - MoveMod(mech))) {
    if (Overwater(mech))
      MechZ(mech) = 0;
    else
      MechZ(mech) = bridge_w_elevation(mech);
    return;
  }
  MechZ(mech) = MechUpperElevation(mech);
}

int DSOkToNotify(Mech *mech) {
  if (DSLastMsg(mech) > mech->xcode.context->events->tick ||
      (mech->xcode.context->events->tick - DSLastMsg(mech)) >= DS_SPAM_TIME) {
    DSLastMsg(mech) = mech->xcode.context->events->tick;
    return 1;
  }
  return 0;
}

int collision_check(Mech *mech, int mode, int le, int lt) {
  int e;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (Overwater(mech) && le < 0)
    le = 0;
  e = MechElevation(mech);
  if (mech_real_terrain_get(mech) == ICE)
    if (le >= 0)
      e = 0;
  if (le < (MechUpperElevation(mech) - MoveMod(mech)) && lt == BRIDGE) {
    if (Overwater(mech))
      le = 0;
    else
      le = bridge_w_elevation(mech);
  }
  if (e < 0 && Overwater(mech))
    e = 0;
  switch (mode) {
  case JUMP:
    if (mech_real_terrain_get(mech) == BRIDGE) {
      if (MechZ(mech) < 0)
        return 1;
      if (MechZ(mech) == (e - 1))
        return 1;
      return 0;
    } else
      return (MechZ(mech) < e);
  case WALK_DROP:
    return (le - e) > MoveMod(mech);
  case WALK_WALL:
    if ((MechMove(mech) == MOVE_HOVER) &&
        (mech_real_terrain_get(mech) == BRIDGE) &&
        (((lt == ICE) || (lt == WATER)) || ((le == 0) && (lt == BRIDGE)))) {
      return 0;
    } else {
      return (e - le) > MoveMod(mech);
    }
  case WALK_BACK:
    if (MechMove(mech) != MOVE_TRACK && MechType(mech) != CLASS_VTOL)
      return (MechSpeed(mech) < 0 ? abs((le - e)) : 0);
    [[fallthrough]];
  case HIT_UNDER_BRIDGE: /* Hovers only... tho it should be fixed for foils and
                            hulls too */
    if ((lt == BRIDGE) && (mech_real_terrain_get(mech) == BRIDGE) &&
        (le == 0) &&
        (Elevation(mech_map, MechLastX(mech), MechLastY(mech)) != 0) &&
        (Elevation(mech_map, MechX(mech), MechY(mech)) == 1))
      return 1;
    else
      return 0;
  }
  return 0;
}

void move_mech(Mech *mech) {
  float newx = 0.0, newy = 0.0, dax, day;
  float xy_charge_dist, xscale;
  float jump_pos;

#ifdef ODDJUMP
  float rjump_pos, midmod;
#endif
  int x, y, upd_z = 0;
  int last_z = 0;
  Mech *target;
  BattleMap *mech_map;
  int oi, oz;
  int iced = 0;

  /* Buffer for printing messages so we don't use the tprintf
   * function anymore */
  char message_buffer[MBUF_SIZE];

  oz = MechZ(mech);
  mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (!mech_map && MechPilot(mech) >= 0)
    mech_map = ValidMap(mech->xcode.context, MechPilot(mech), mech->mapindex);

  /* Unit is not on a map so don't need to move it - instead
   * reset its values and shut it down */
  if (!mech_map) {

    mech_notify(mech, MECHALL, "You are on an invalid map! Map index reset!");
    MechCocoon(mech) = 0;

    if (Jumping(mech))
      mech_land(MechPilot(mech), (void *)mech, "");

    mech_shutdown(MechPilot(mech), (void *)mech, "");

    snprintf(message_buffer, MBUF_SIZE,
             "move_mech:invalid map:Mech: %ld Index: %ld", mech->mynum,
             mech->mapindex);
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       message_buffer);
    mech->mapindex = -1;
    return;
  }

  /* Is the unit on a valid spot on the map */
  if (MechX(mech) < 0 || MechX(mech) >= mech_map->map_width ||
      MechLastX(mech) < 0 || MechLastX(mech) >= mech_map->map_width ||
      MechY(mech) < 0 || MechY(mech) >= mech_map->map_height ||
      MechLastY(mech) < 0 || MechLastY(mech) >= mech_map->map_height) {

    mech_notify(mech, MECHALL,
                "You are at an invalid map location! Map index reset!");
    MechCocoon(mech) = 0;

    if (Jumping(mech))
      mech_land(MechPilot(mech), (void *)mech, "");

    mech_shutdown(MechPilot(mech), (void *)mech, "");

    snprintf(message_buffer, MBUF_SIZE,
             "move_mech:invalid map:Mech: %ld Index: %ld", mech->mynum,
             mech->mapindex);
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       message_buffer);

    mech->mapindex = -1;
    return;
  }

  /* Is the unit charging - and if so have they been charging to long */
  if (mech->xcode.context->configuration->btech_newcharge &&
      MechChargeTarget(mech) > 0) {
    if (MechChargeTimer(mech)++ > CHARGE_TIMER_LIMIT) {
      mech_notify(mech, MECHALL, "Charge timed out, charge reset.");
      MechChargeTarget(mech) = -1;
      MechChargeTimer(mech) = 0;
      MechChargeDistance(mech) = 0;
    }
  }

  /*! \todo {now that I think about it maybe make a single block
   * of code to check for jumping BEFORE anything else.} */

  /* Check the move type of the unit */
  switch (MechMove(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD:

    /* Biped/Quad Jumping */
    if (Jumping(mech)) {
      MarkForLOSUpdate(mech);
      FindComponents(JumpSpeed(mech, mech_map) * MOVE_MOD *
                         MAPMOVEMOD(mech_map),
                     MechJumpHeading(mech), &newx, &newy);
      MechFX(mech) += newx;
      MechFY(mech) += newy;
      jump_pos = length_hypotenuse((MechFX(mech) - MechStartFX(mech)),
                                   (MechFY(mech) - MechStartFY(mech)));

      /*! \todo {Not sure what ODDJUMP is but need to figure it out, possibly
       * make this a configuration parameter} */
#ifndef ODDJUMP
      MechFZ(mech) = ((4 * JumpSpeedMP(mech, mech_map) * ZSCALE) /
                      (MechJumpLength(mech) * MechJumpLength(mech))) *
                         jump_pos * (MechJumpLength(mech) - jump_pos) +
                     MechStartFZ(mech) +
                     jump_pos * (MechEndFZ(mech) - MechStartFZ(mech)) /
                         (MechJumpLength(mech) * HEXLEVEL);
#else
      rjump_pos = MechJumpLength(mech) - jump_pos;
      if (rjump_pos < 0.0)
        rjump_pos = 0.0;

      /* New flight path: Make a direct line from the origin to
         destination, and imagine a 1-x^4 in relation to the 0 as
         the line. y=1 = JumpTop offset, x=0 = middle of path,
         x=-1 = beginning, x=1 = end */
      midmod = jump_pos / MechJumpLength(mech);
      midmod = (midmod - 0.5) * 2;
      if (MechJumpTop(mech) >= (1 + JumpSpeedMP(mech, mech_map))) {
        midmod = (1.0 - (midmod * midmod)) * MechJumpTop(mech);
      } else {
        midmod =
            (1.0 - (midmod * midmod * midmod * midmod)) * MechJumpTop(mech);
      }

      MechFZ(mech) =
          (rjump_pos * MechStartFZ(mech) + jump_pos * MechEndFZ(mech)) /
              MechJumpLength(mech) +
          midmod * ZSCALE;
#endif

      MechZ(mech) = (int)(MechFZ(mech) / ZSCALE + 0.5);

#ifdef JUMPDEBUG
      snprintf(message_buffer, MBUF_SIZE, "#%d: %d, %d, %d (%d, %d, %d)",
               mech->mynum, MechX(mech), MechY(mech), MechZ(mech),
               (int)MechFX(mech), (int)MechFY(mech), (int)MechFZ(mech));
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                         message_buffer);
#endif

      /* The famous jumping on bridge collision code */
      /*! \todo {possibly turn the bridge collision code into
       * a configuration parameter} */
      if (mech_real_terrain_get(mech) == BRIDGE &&
          collision_check(mech, JUMP, 0, 0) && MechZ(mech) > 0) {

        mech_notify(mech, MECHALL, "CRASH! You crash into the bridge!");
        mech_los_broadcast(mech, "crashes into the bridge!");
        mech_fall(mech, 1, 0);
        return;
      }

      /* Is the jumping unit in its target hex */
      if ((MechX(mech) == MechGoingX(mech)) &&
          (MechY(mech) == MechGoingY(mech))) {

        /*Ok.. in the hex. but no instant landings anymore, laddie */
        /*Range to point of origin is larger than whole jump's length */
        MapCoordToRealCoord(MechX(mech), MechY(mech), &dax, &day);

        /*! \todo {Another instance of that crazy ODDJUMP stuff} */
        /* Basicly checking to see if the unit is in the hex,
         * the ODDJUMP code looks for the exact center */
#ifdef ODDJUMP
        if (length_hypotenuse(dax - MechStartFX(mech),
                              day - MechStartFY(mech)) <=
            length_hypotenuse(MechFX(mech) - MechStartFX(mech),
                              MechFY(mech) - MechStartFY(mech))) {

          mech_jump_land(mech);
          MechFX(mech) = (float)dax;
          MechFY(mech) = (float)day;
        }
#else
        mech_jump_land(mech);
        MechFX(mech) = (float)dax;
        MechFY(mech) = (float)day;
#endif
      }

      /* Are we landing on ice */
      if (mech_real_terrain_get(mech) == ICE) {
        if (oz < -1 && MechZ(mech) >= -1)
          break_thru_ice(mech);
        else if (oz >= -1 && MechZ(mech) < -1)
          drop_thru_ice(mech);
      }

    } else if (fabs(MechSpeed(mech)) > 0.0) {

      /* Ok not jumping but we're moving */
      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                     MechLateral(mech) + MechFacing(mech), &newx, &newy);
      MechFX(mech) += newx;
      MechFY(mech) += newy;

      upd_z = 1;

      /* If we're charging record distance traveled */
      if (MechChargeTarget(mech) > 0 &&
          mech->xcode.context->configuration->btech_newcharge) {
        xscale = 1.0 / SCALEMAP;
        xscale = xscale * xscale;
        xy_charge_dist = sqrt(xscale * newx * newx + YSCALE2 * newy * newy);
        MechChargeDistance(mech) += xy_charge_dist;
      }

    } else {

      /* Ok not moving or jumping so don't need to calc a new x,y,z */
      return;
    }
    break;

  case MOVE_TRACK:
  case MOVE_WHEEL:

    /*! \todo {Put the tank JJ code here - for tracked/wheeled} */

    /* Is the tank moving? */
    if (fabs(MechSpeed(mech)) > 0.0) {

      /*! \todo {Possibly put in a configuration parameter for this since
       * I think the LATERAL command could check for it} */
#ifndef BT_MOVEMENT_MODES
      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                     MechFacing(mech), &newx, &newy);
#else
      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                     MechLateral(mech) + MechFacing(mech), &newx, &newy);
#endif
      MechFX(mech) += newx;
      MechFY(mech) += newy;
      upd_z = 1;

      /* If we're charging record the distance traveled */
      if (MechChargeTarget(mech) > 0 &&
          mech->xcode.context->configuration->btech_newcharge) {
        xscale = 1.0 / SCALEMAP;
        xscale = xscale * xscale;
        xy_charge_dist = sqrt(xscale * newx * newx + YSCALE2 * newy * newy);
        MechChargeDistance(mech) += xy_charge_dist;
      }

    } else {

      /* Ok not moving or jumping so don't need to calc a new x,y,z */
      return;
    }
    break;

  case MOVE_HOVER:

    /* If we're moving update position */
    if (fabs(MechSpeed(mech)) > 0.0) {

      /*! \todo {Check the todo before this one, configuration parameter?} */
#ifndef BT_MOVEMENT_MODES
      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                     MechFacing(mech), &newx, &newy);
#else
      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                     MechLateral(mech) + MechFacing(mech), &newx, &newy);
#endif
      MechFX(mech) += newx;
      MechFY(mech) += newy;
      upd_z = 1;

      /* Record the charge distance */
      if (MechChargeTarget(mech) > 0 &&
          mech->xcode.context->configuration->btech_newcharge) {
        xscale = 1.0 / SCALEMAP;
        xscale = xscale * xscale;
        xy_charge_dist = sqrt(xscale * newx * newx + YSCALE2 * newy * newy);
        MechChargeDistance(mech) += xy_charge_dist;
      }

    } else {

      /* Ok not moving or jumping so don't need to calc a new x,y,z */
      return;
    }
    break;

  case MOVE_VTOL:

    /* If we're landed we're not moving */
    if (Landed(mech))
      return;

    /* Use the same code as subs below */
    /*! \todo {VTOLS not able to lateral, nor can subs right now} */

    [[fallthrough]];
  case MOVE_SUB:

    MarkForLOSUpdate(mech);
    FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                   MechFacing(mech), &newx, &newy);
    MechFX(mech) += newx;
    MechFY(mech) += newy;
    MechFZ(mech) += MechVerticalSpeed(mech) * MOVE_MOD;
    MechZ(mech) = MechFZ(mech) / ZSCALE;
    break;

  case MOVE_FLY:

    /* Ok we're in the air */
    if (!Landed(mech)) {

      MarkForLOSUpdate(mech);
      MechFZ(mech) += MechStartFZ(mech) * MOVE_MOD;
      MechZ(mech) = MechFZ(mech) / ZSCALE;
      MechFX(mech) += MechStartFX(mech) * MOVE_MOD;
      MechFY(mech) += MechStartFY(mech) * MOVE_MOD;

      /*! \todo {Need to rewrite this for aerodyne DSes unless they're
       * set as large aeros which i'm not sure but either way its
       * probably not good} */
      if (IsDS(mech)) {

        /* Fun messages to emit to the pilot */
        if (MechZ(mech) < 10 && oz >= 10)
          DS_LandWarning(mech, 1);
        else if (MechZ(mech) < 50 && oz >= 50)
          DS_LandWarning(mech, 0);
        else if (MechZ(mech) < 100 && oz >= 100) {

          if (abs(MechDesiredAngle(mech)) != 90) {
            if (DSOkToNotify(mech)) {
              mech_notify(mech, MECHALL,
                          "As the craft enters "
                          "the lower atmosphere, its nose rises up "
                          "for a clean landing..");
              snprintf(message_buffer, MBUF_SIZE,
                       "starts descending towards %d, %d..", MechX(mech),
                       MechY(mech));
              mech_los_broadcast(mech, message_buffer);
            } else {
              mech_notify(mech, MECHALL,
                          "Due to low altitude, "
                          "climbing angle set to 90 degrees.");
            }
            MechDesiredAngle(mech) = 90;
          }
          MechStartFX(mech) = 0.0;
          MechStartFY(mech) = 0.0;
          DS_LandWarning(mech, -1);
        }
      }

    } else {

      /* Ok we're rolling around on the ground */
      if (!(fabs(MechSpeed(mech)) > 0.0))
        return;

      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                     MechFacing(mech), &newx, &newy);
      MechFX(mech) += newx;
      MechFY(mech) += newy;
      upd_z = 1;
    }
    break;

  case MOVE_HULL:
  case MOVE_FOIL:

    if (fabs(MechSpeed(mech)) > 0.0) {

      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(mech_map),
                     MechFacing(mech), &newx, &newy);
      MechFX(mech) += newx;
      MechFY(mech) += newy;
      MechZ(mech) = 0;
      MechFZ(mech) = 0.0;

    } else {

      /* Not moving so no need to update x,y,z and other stuff */
      return;
    }
    break;
  }

  /* We've already updated the floating x,y values, now to update the hex
   * x,y values + both the Z values */
  MechLastX(mech) = MechX(mech);
  MechLastY(mech) = MechY(mech);
  last_z = MechZ(mech);

  RealCoordToMapCoord(&MechX(mech), &MechY(mech), MechFX(mech), MechFY(mech));

  /*! \todo {Its the ODDJUMP guy again, whats it for?} */
  /* Checking to make sure we didn't jump PAST our target hex */
#ifdef ODDJUMP
  if (Jumping(mech) && MechLastX(mech) == MechGoingX(mech) &&
      MechLastY(mech) == MechGoingY(mech) &&
      (MechX(mech) != MechLastX(mech) || MechY(mech) != MechLastY(mech))) {

    mech_jump_land(mech);
    MechFX(mech) -= newx;
    MechFY(mech) -= newy;
    MechFZ(mech) = MechEndFZ(mech);
    MechX(mech) = MechGoingX(mech);
    MechY(mech) = MechGoingY(mech);
    MapCoordToRealCoord(MechX(mech), MechY(mech), &MechFX(mech), &MechFY(mech));
    MechZ(mech) = MechFZ(mech) / ZSCALE;
  }
#endif

  /* Store the current map index */
  oi = mech->mapindex;

  /* Did we hit mapedge? */
  CheckEdgeOfMap(mech);

  /* Did our mapindex change? - like did we run out a hangar? */
  if (mech->mapindex != oi)
    mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  /* We left a hangar and/or moved to a new hex */
  if (oi != mech->mapindex || MechLastX(mech) != MechX(mech) ||
      MechLastY(mech) != MechY(mech)) {

    /* Either the mech or the map is bad */
    if (!mech || !mech_map) {

      snprintf(message_buffer, MBUF_SIZE,
               "Invalide pointer (%s) in move_mech()",
               (!mech       ? "mech"
                : !mech_map ? "mech_map"
                            : "weird...."));
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         message_buffer);

      if (mech) {

        /* Bad Map */
        mech_notify(mech, MECHALL,
                    "You are on an invalid map! Map index reset!");
        MechCocoon(mech) = 0;

        if (Jumping(mech))
          mech_land(MechPilot(mech), (void *)mech, "");

        mech_shutdown(MechPilot(mech), (void *)mech, "");
        snprintf(message_buffer, MBUF_SIZE,
                 "move_mech:invalid map:Mech: %ld Index: %ld", mech->mynum,
                 mech->mapindex);
        btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                           message_buffer);
        mech->mapindex = -1;
      }
      return;
    }

    /* We've moved from our hex so break our cover */
    if (MechCritStatus(mech) & HIDDEN) {
      mech_notify(mech, MECHALL, "You move too much and break your cover!");
      mech_los_broadcast(mech, "breaks from its cover.");
      MechCritStatus(mech) &= ~(HIDDEN);
    }

    mech_event_cancel(mech, EVENT_HIDE);

    x = MechX(mech);
    y = MechY(mech);
    MechTerrain(mech) = map_terrain_get(mech_map, x, y);
    MechElev(mech) = map_elevation_get(mech_map, x, y);

    /* Update our Z values */
    if (upd_z) {

      /* Chance of breaking through ICE */
      if (mech_real_terrain_get(mech) == ICE) {
        if (oz < -1 && MechZ(mech) >= -1)
          break_thru_ice(mech);
        else if (MechZ(mech) == 0)
          if (possibly_drop_thru_ice(mech))
            iced = 1;
      }

      /* Check for bridges and basic elevation changes */
      mech_drop_surface_set(mech, false);

      /* To fix certain slide-under-ice-effect for _mechs_ */
      if (MechType(mech) == CLASS_MECH && mech_real_terrain_get(mech) == ICE &&
          oz == -1 && MechZ(mech) == -1) {

        MechZ(mech) = 0;
        MechFZ(mech) = MechZ(mech) * ZSCALE;
      }
    }

    if (!iced)
      NewHexEntered(mech, mech_map, newx, newy, last_z);

    if (MechX(mech) == x && MechY(mech) == y) {
      // Removed MarkForLOSUpdate.. Moved to NewHexEntered to clear potential
      // false positives.
      //			MarkForLOSUpdate(mech);
      mech_flood(mech);
      water_extinguish_inferno(mech);
      steppable_base_check(mech, x, y);

      /* Pilot XP */
      if (is_in_character(mech->xcode.context->database, mech->mynum)) {
        MechHexes(mech)++;
        if (!((int)MechHexes(mech) % PIL_XP_EVERY_N_STEPS))
          if (mech_has_active_pilot(mech))
            AccumulatePilXP(MechPilot(mech), mech, 1, 0);
      }

      /* Check for stacking */
      mech_domino_resolve(mech, MECH_DOMINO_GROUND);
    }
  }

  /* If aero/vtol make sure we're not rubbing against the ground or trees */
  if ((MechMove(mech) == MOVE_VTOL || is_aero(mech)) && !Landed(mech))
    mech_vtol_altitude_check(mech);

  /* If we're a boat make sure we've not run a ground */
  if (MechType(mech) == CLASS_VEH_NAVAL)
    mech_naval_altitude_check(mech, oz);

  /* We're charging lets do some damage */
  if (MechChargeTarget(mech) != -1) {

    /* Valid target? */
    target =
        btech_context_get_mech(mech->xcode.context, MechChargeTarget(mech));
    if (target) {

      if (FaMechRange(mech, target) < CHARGE_DIST_TRIGGER) {
        ChargeMech(mech, target);
        MechChargeTarget(mech) = -1;
        MechChargeTimer(mech) = 0;
        MechChargeDistance(mech) = 0;
      }

    } else {
      mech_notify(mech, MECHPILOT, "Invalid CHARGE target!");
      MechChargeTarget(mech) = -1;
      MechChargeDistance(mech) = 0;
      MechChargeTimer(mech) = 0;
    }
  }

  /* If we're towing something update its position with us */
  if (MechCarrying(mech) > 0) {
    target = btech_context_get_mech(mech->xcode.context, MechCarrying(mech));
    if (target && target->mapindex == mech->mapindex) {
      MirrorPosition(mech, target, 0);
      SetRFacing(target, MechRFacing(mech));
    }
  }

  /* If a bsuit has swarmed a target update its
   * position in relation to its target */
  BSuitMirrorSwarmedTarget(mech_map, mech);

  /* This is really for killing MechWarriors
   * actual heat code is checked with mech_heat_update */
  fiery_death(mech);
}
