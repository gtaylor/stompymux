#include "mech_motion_integration_api.h"

#include "mech_update_internal.h"

#include "mech_charge_tracking_api.h"

bool mech_motion_integrate(Mech *mech, BattleMap *map, MechMotionStep *step) {
  float jump_position;
  float target_x;
  float target_y;

#ifdef ODDJUMP
  float remaining_jump;
  float midpoint_modifier;
#endif

  char message_buffer[MBUF_SIZE];

  *step = (MechMotionStep){.previous_z = MechZ(mech)};

  switch (MechMove(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD:
    if (Jumping(mech)) {
      MarkForLOSUpdate(mech);
      FindComponents(JumpSpeed(mech, map) * MOVE_MOD * MAPMOVEMOD(map),
                     MechJumpHeading(mech), &step->delta_x, &step->delta_y);
      MechFX(mech) += step->delta_x;
      MechFY(mech) += step->delta_y;
      jump_position = length_hypotenuse(MechFX(mech) - MechStartFX(mech),
                                        MechFY(mech) - MechStartFY(mech));

#ifndef ODDJUMP
      MechFZ(mech) = ((4 * JumpSpeedMP(mech, map) * ZSCALE) /
                      (MechJumpLength(mech) * MechJumpLength(mech))) *
                         jump_position *
                         (MechJumpLength(mech) - jump_position) +
                     MechStartFZ(mech) +
                     jump_position * (MechEndFZ(mech) - MechStartFZ(mech)) /
                         (MechJumpLength(mech) * HEXLEVEL);
#else
      remaining_jump = MechJumpLength(mech) - jump_position;
      if (remaining_jump < 0.0)
        remaining_jump = 0.0;

      midpoint_modifier = jump_position / MechJumpLength(mech);
      midpoint_modifier = (midpoint_modifier - 0.5) * 2;
      if (MechJumpTop(mech) >= (1 + JumpSpeedMP(mech, map))) {
        midpoint_modifier =
            (1.0 - (midpoint_modifier * midpoint_modifier)) * MechJumpTop(mech);
      } else {
        midpoint_modifier = (1.0 - (midpoint_modifier * midpoint_modifier *
                                    midpoint_modifier * midpoint_modifier)) *
                            MechJumpTop(mech);
      }

      MechFZ(mech) = (remaining_jump * MechStartFZ(mech) +
                      jump_position * MechEndFZ(mech)) /
                         MechJumpLength(mech) +
                     midpoint_modifier * ZSCALE;
#endif

      MechZ(mech) = (int)(MechFZ(mech) / ZSCALE + 0.5);

#ifdef JUMPDEBUG
      snprintf(message_buffer, MBUF_SIZE, "#%d: %d, %d, %d (%d, %d, %d)",
               mech->mynum, MechX(mech), MechY(mech), MechZ(mech),
               (int)MechFX(mech), (int)MechFY(mech), (int)MechFZ(mech));
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                         message_buffer);
#endif

      if (mech_real_terrain_get(mech) == BRIDGE &&
          collision_check(mech, JUMP, 0, 0) && MechZ(mech) > 0) {
        mech_notify(mech, MECHALL, "CRASH! You crash into the bridge!");
        mech_los_broadcast(mech, "crashes into the bridge!");
        mech_fall(mech, 1, 0);
        return false;
      }

      if (MechX(mech) == MechGoingX(mech) && MechY(mech) == MechGoingY(mech)) {
        MapCoordToRealCoord(MechX(mech), MechY(mech), &target_x, &target_y);
#ifdef ODDJUMP
        if (length_hypotenuse(target_x - MechStartFX(mech),
                              target_y - MechStartFY(mech)) <=
            length_hypotenuse(MechFX(mech) - MechStartFX(mech),
                              MechFY(mech) - MechStartFY(mech))) {
          mech_jump_land(mech);
          MechFX(mech) = target_x;
          MechFY(mech) = target_y;
        }
#else
        mech_jump_land(mech);
        MechFX(mech) = target_x;
        MechFY(mech) = target_y;
#endif
      }

      if (mech_real_terrain_get(mech) == ICE) {
        if (step->previous_z < -1 && MechZ(mech) >= -1)
          break_thru_ice(mech);
        else if (step->previous_z >= -1 && MechZ(mech) < -1)
          drop_thru_ice(mech);
      }
    } else if (fabs(MechSpeed(mech)) > 0.0) {
      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                     MechLateral(mech) + MechFacing(mech), &step->delta_x,
                     &step->delta_y);
      MechFX(mech) += step->delta_x;
      MechFY(mech) += step->delta_y;
      step->update_surface = true;
      mech_charge_distance_record(mech, step->delta_x, step->delta_y);
    } else {
      return false;
    }
    break;

  case MOVE_TRACK:
  case MOVE_WHEEL:
    if (fabs(MechSpeed(mech)) <= 0.0)
      return false;
#ifndef BT_MOVEMENT_MODES
    FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                   MechFacing(mech), &step->delta_x, &step->delta_y);
#else
    FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                   MechLateral(mech) + MechFacing(mech), &step->delta_x,
                   &step->delta_y);
#endif
    MechFX(mech) += step->delta_x;
    MechFY(mech) += step->delta_y;
    step->update_surface = true;
    mech_charge_distance_record(mech, step->delta_x, step->delta_y);
    break;

  case MOVE_HOVER:
    if (fabs(MechSpeed(mech)) <= 0.0)
      return false;
#ifndef BT_MOVEMENT_MODES
    FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                   MechFacing(mech), &step->delta_x, &step->delta_y);
#else
    FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                   MechLateral(mech) + MechFacing(mech), &step->delta_x,
                   &step->delta_y);
#endif
    MechFX(mech) += step->delta_x;
    MechFY(mech) += step->delta_y;
    step->update_surface = true;
    mech_charge_distance_record(mech, step->delta_x, step->delta_y);
    break;

  case MOVE_VTOL:
    if (Landed(mech))
      return false;
    [[fallthrough]];
  case MOVE_SUB:
    MarkForLOSUpdate(mech);
    FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                   MechFacing(mech), &step->delta_x, &step->delta_y);
    MechFX(mech) += step->delta_x;
    MechFY(mech) += step->delta_y;
    MechFZ(mech) += MechVerticalSpeed(mech) * MOVE_MOD;
    MechZ(mech) = MechFZ(mech) / ZSCALE;
    break;

  case MOVE_FLY:
    if (!Landed(mech)) {
      MarkForLOSUpdate(mech);
      MechFZ(mech) += MechStartFZ(mech) * MOVE_MOD;
      MechZ(mech) = MechFZ(mech) / ZSCALE;
      MechFX(mech) += MechStartFX(mech) * MOVE_MOD;
      MechFY(mech) += MechStartFY(mech) * MOVE_MOD;

      if (IsDS(mech)) {
        if (MechZ(mech) < 10 && step->previous_z >= 10)
          DS_LandWarning(mech, 1);
        else if (MechZ(mech) < 50 && step->previous_z >= 50)
          DS_LandWarning(mech, 0);
        else if (MechZ(mech) < 100 && step->previous_z >= 100) {
          if (abs(MechDesiredAngle(mech)) != 90) {
            if (dropship_notification_is_due(mech)) {
              mech_notify(mech, MECHALL,
                          "As the craft enters the lower atmosphere, its nose "
                          "rises up for a clean landing..");
              snprintf(message_buffer, MBUF_SIZE,
                       "starts descending towards %d, %d..", MechX(mech),
                       MechY(mech));
              mech_los_broadcast(mech, message_buffer);
            } else {
              mech_notify(mech, MECHALL,
                          "Due to low altitude, climbing angle set to 90 "
                          "degrees.");
            }
            MechDesiredAngle(mech) = 90;
          }
          MechStartFX(mech) = 0.0;
          MechStartFY(mech) = 0.0;
          DS_LandWarning(mech, -1);
        }
      }
    } else {
      if (fabs(MechSpeed(mech)) <= 0.0)
        return false;
      FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                     MechFacing(mech), &step->delta_x, &step->delta_y);
      MechFX(mech) += step->delta_x;
      MechFY(mech) += step->delta_y;
      step->update_surface = true;
    }
    break;

  case MOVE_HULL:
  case MOVE_FOIL:
    if (fabs(MechSpeed(mech)) <= 0.0)
      return false;
    FindComponents(MechSpeed(mech) * MOVE_MOD * MAPMOVEMOD(map),
                   MechFacing(mech), &step->delta_x, &step->delta_y);
    MechFX(mech) += step->delta_x;
    MechFY(mech) += step->delta_y;
    MechZ(mech) = 0;
    MechFZ(mech) = 0.0;
    break;
  }

  return true;
}
