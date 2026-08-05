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

#include "mech_charge_tracking_api.h"
#include "mech_motion_integration_api.h"
#include "mech_movement_validation_api.h"
#include "mech_towing_sync_api.h"

void mech_movement_update(Mech *mech) {
  BattleMap *mech_map = mech_movement_map_validate(mech);
  if (!mech_map)
    return;

  mech_charge_timeout_update(mech);

  MechMotionStep step;
  if (!mech_motion_integrate(mech, mech_map, &step))
    return;

  int last_z = MechZ(mech);
  MechLastX(mech) = MechX(mech);
  MechLastY(mech) = MechY(mech);
  RealCoordToMapCoord(&MechX(mech), &MechY(mech), MechFX(mech), MechFY(mech));

#ifdef ODDJUMP
  if (Jumping(mech) && MechLastX(mech) == MechGoingX(mech) &&
      MechLastY(mech) == MechGoingY(mech) &&
      (MechX(mech) != MechLastX(mech) || MechY(mech) != MechLastY(mech))) {
    mech_jump_land(mech);
    MechFX(mech) -= step.delta_x;
    MechFY(mech) -= step.delta_y;
    MechFZ(mech) = MechEndFZ(mech);
    MechX(mech) = MechGoingX(mech);
    MechY(mech) = MechGoingY(mech);
    MapCoordToRealCoord(MechX(mech), MechY(mech), &MechFX(mech), &MechFY(mech));
    MechZ(mech) = MechFZ(mech) / ZSCALE;
  }
#endif

  int previous_map = mech->mapindex;
  CheckEdgeOfMap(mech);
  if (mech->mapindex != previous_map)
    mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (previous_map != mech->mapindex || MechLastX(mech) != MechX(mech) ||
      MechLastY(mech) != MechY(mech)) {
    if (!mech || !mech_map) {
      char message_buffer[MBUF_SIZE];
      snprintf(message_buffer, MBUF_SIZE,
               "Invalide pointer (%s) in move_mech()",
               (!mech       ? "mech"
                : !mech_map ? "mech_map"
                            : "weird...."));
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         message_buffer);

      if (mech) {
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

    if (MechCritStatus(mech) & HIDDEN) {
      mech_notify(mech, MECHALL, "You move too much and break your cover!");
      mech_los_broadcast(mech, "breaks from its cover.");
      MechCritStatus(mech) &= ~HIDDEN;
    }
    mech_event_cancel(mech, EVENT_HIDE);

    int x = MechX(mech);
    int y = MechY(mech);
    MechTerrain(mech) = map_terrain_get(mech_map, x, y);
    MechElev(mech) = map_elevation_get(mech_map, x, y);

    int iced = 0;
    if (step.update_surface) {
      if (mech_real_terrain_get(mech) == ICE) {
        if (step.previous_z < -1 && MechZ(mech) >= -1)
          break_thru_ice(mech);
        else if (MechZ(mech) == 0 && possibly_drop_thru_ice(mech))
          iced = 1;
      }

      mech_drop_surface_set(mech, false);
      if (MechType(mech) == CLASS_MECH && mech_real_terrain_get(mech) == ICE &&
          step.previous_z == -1 && MechZ(mech) == -1) {
        MechZ(mech) = 0;
        MechFZ(mech) = MechZ(mech) * ZSCALE;
      }
    }

    if (!iced)
      mech_hex_entry_resolve(mech, mech_map, step.delta_x, step.delta_y,
                             last_z);

    if (MechX(mech) == x && MechY(mech) == y) {
      mech_flood(mech);
      water_extinguish_inferno(mech);
      steppable_base_check(mech, x, y);

      if (is_in_character(mech->xcode.context->database, mech->mynum)) {
        MechHexes(mech)++;
        if (!((int)MechHexes(mech) % PIL_XP_EVERY_N_STEPS) &&
            mech_has_active_pilot(mech))
          AccumulatePilXP(MechPilot(mech), mech, 1, 0);
      }

      mech_domino_resolve(mech, MECH_DOMINO_GROUND);
    }
  }

  if ((MechMove(mech) == MOVE_VTOL || is_aero(mech)) && !Landed(mech))
    mech_vtol_altitude_check(mech);
  if (MechType(mech) == CLASS_VEH_NAVAL)
    mech_naval_altitude_check(mech, step.previous_z);

  mech_charge_impact_resolve(mech);
  mech_towing_position_update(mech);
  BSuitMirrorSwarmedTarget(mech_map, mech);
  mech_fire_hazard_resolve(mech);
}
