/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

int fasa_ground_hit_location(Mech *mech, int hitGroup, int *iscritical,
                             int *isrear, int roll) {
  int hitloc = 0;
  int side;

  switch (MechType(mech)) {
  case CLASS_VEH_GROUND:
    switch (hitGroup) {

    case LEFTSIDE:
      switch (roll) {
      case 2:
        /* A Roll on Determining Critical Hits Table */
        *iscritical = 1;
        return LSIDE;
      case 3:
        if (mech->xcode.context->configuration->btech_tankfriendly) {
          if (!Fallen(mech)) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch (MechMove(mech)) {
            case MOVE_TRACK:
              mech_notify(mech, MECHALL,
                          "One of your tracks is seriously damaged!");
              break;
            case MOVE_WHEEL:
              mech_notify(mech, MECHALL,
                          "One of your wheels is seriously damaged!");
              break;
            case MOVE_HOVER:
              mech_notify(mech, MECHALL,
                          "Your air skirt is seriously damaged!");
              break;
            case MOVE_HULL:
            case MOVE_SUB:
            case MOVE_FOIL:
              mech_notify(
                  mech, MECHALL,
                  "Your craft lurches and suddenly loses a lot of speed!");
              break;
            }
            mech_max_speed_lower(mech, MP2);
          }
          return LSIDE;
        }
        /* Cripple tank */
        if (!Fallen(mech)) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch (MechMove(mech)) {
          case MOVE_TRACK:
            mech_notify(
                mech, MECHALL,
                "One of your tracks is destroyed, immobilizing your vehicle!");
            break;
          case MOVE_WHEEL:
            mech_notify(
                mech, MECHALL,
                "One of your wheels is destroyed, immobilizing your vehicle!");
            break;
          case MOVE_HOVER:
            mech_notify(
                mech, MECHALL,
                "Your lift fan is destroyed, immobilizing your vehicle!");
            break;
          case MOVE_HULL:
          case MOVE_SUB:
          case MOVE_FOIL:
            mech_notify(mech, MECHALL,
                        "Your engines cut out and you drift to a halt!");
          }
          mech_max_speed_set(mech, 0.0);

          mech_make_fall(mech);
        }
        return LSIDE;
      case 4:
      case 5:
        /* MP -1 */
        if (!Fallen(mech)) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch (MechMove(mech)) {
          case MOVE_TRACK:
            mech_notify(mech, MECHALL, "One of your tracks is damaged!");
            break;
          case MOVE_WHEEL:
            mech_notify(mech, MECHALL, "One of your wheels is damaged!");
            break;
          case MOVE_HOVER:
            mech_notify(mech, MECHALL, "Your air skirt is damaged!");
            break;
          case MOVE_HULL:
          case MOVE_SUB:
          case MOVE_FOIL:
            mech_notify(mech, MECHALL, "Your craft suddenly slows!");
            break;
          }
          mech_max_speed_lower(mech, MP1);
        }
        return LSIDE;
        break;
      case 6:
      case 7:
      case 8:
      case 9:
        /* MP -1 if hover */
        return LSIDE;
      case 10:
        return (GetSectInt(mech, TURRET)) ? TURRET : LSIDE;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          if (!(MechTankCritStatus(mech) & TURRET_LOCKED)) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            MechTankCritStatus(mech) |= TURRET_LOCKED;
            mech_notify(mech, MECHALL,
                        "Your turret takes a direct hit and locks up!");
          }
          return TURRET;
        } else
          return LSIDE;
      case 12:
        /* A Roll on Determining Critical Hits Table */
        *iscritical = 1;
        return LSIDE;
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
        *iscritical = 1;
        return RSIDE;
      case 3:
        if (mech->xcode.context->configuration->btech_tankfriendly) {
          if (!Fallen(mech)) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch (MechMove(mech)) {
            case MOVE_TRACK:
              mech_notify(mech, MECHALL,
                          "One of your tracks is seriously damaged!");
              break;
            case MOVE_WHEEL:
              mech_notify(mech, MECHALL,
                          "One of your wheels is seriously damaged!");
              break;
            case MOVE_HOVER:
              mech_notify(mech, MECHALL,
                          "Your air skirt is seriously damaged!");
              break;
            case MOVE_HULL:
            case MOVE_SUB:
            case MOVE_FOIL:
              mech_notify(
                  mech, MECHALL,
                  "Your craft lurches and suddenly loses a lot of speed!");
              break;
            }
            mech_max_speed_lower(mech, MP2);
          }
          return RSIDE;
        }
        /* Cripple Tank */
        if (!Fallen(mech)) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch (MechMove(mech)) {
          case MOVE_TRACK:
            mech_notify(
                mech, MECHALL,
                "One of your tracks is destroyed, immobilizing your vehicle!");
            break;
          case MOVE_WHEEL:
            mech_notify(
                mech, MECHALL,
                "One of your wheels is destroyed, immobilizing your vehicle!");
            break;
          case MOVE_HOVER:
            mech_notify(
                mech, MECHALL,
                "Your lift fan is destroyed, immobilizing your vehicle!");
            break;
          case MOVE_HULL:
          case MOVE_SUB:
          case MOVE_FOIL:
            mech_notify(mech, MECHALL,
                        "Your engines cut out and you drift to a halt!");
          }
          mech_max_speed_set(mech, 0.0);

          mech_make_fall(mech);
        }
        return RSIDE;
      case 4:
      case 5:
        /* MP -1 */
        if (!Fallen(mech)) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch (MechMove(mech)) {
          case MOVE_TRACK:
            mech_notify(mech, MECHALL, "One of your tracks is damaged!");
            break;
          case MOVE_WHEEL:
            mech_notify(mech, MECHALL, "One of your wheels is damaged!");
            break;
          case MOVE_HOVER:
            mech_notify(mech, MECHALL, "Your air skirt is damaged!");
            break;
          case MOVE_HULL:
          case MOVE_SUB:
          case MOVE_FOIL:
            mech_notify(mech, MECHALL, "Your craft suddenly slows!");
            break;
          }
          mech_max_speed_lower(mech, MP1);
        }
        return RSIDE;
      case 6:
      case 7:
      case 8:
        return RSIDE;
      case 9:
        /* MP -1 if hover */
        if (!Fallen(mech)) {
          if (MechMove(mech) == MOVE_HOVER) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            mech_notify(mech, MECHALL, "Your air skirt is damaged!");
            mech_max_speed_lower(mech, MP1);
          }
        }
        return RSIDE;
      case 10:
        return (GetSectInt(mech, TURRET)) ? TURRET : RSIDE;
      case 11:
        if (GetSectInt(mech, TURRET)) {
          if (!(MechTankCritStatus(mech) & TURRET_LOCKED)) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            MechTankCritStatus(mech) |= TURRET_LOCKED;
            mech_notify(mech, MECHALL,
                        "Your turret takes a direct hit and locks up!");
          }
          return TURRET;
        } else
          return RSIDE;
      case 12:
        /* A Roll on Determining Critical Hits Table */
        *iscritical = 1;
        return RSIDE;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);
      switch (roll) {
      case 2:
        /* A Roll on Determining Critical Hits Table */
        *iscritical = 1;
        return side;
      case 3:
        if (mech->xcode.context->configuration->btech_tankshield) {
          if (mech->xcode.context->configuration->btech_tankfriendly) {
            if (!Fallen(mech)) {
              mech_notify(mech, MECHALL,
                          "[fg=yellow bold]CRITICAL HIT![reset]");
              switch (MechMove(mech)) {
              case MOVE_TRACK:
                mech_notify(mech, MECHALL,
                            "One of your tracks is seriously damaged!");
                break;
              case MOVE_WHEEL:
                mech_notify(mech, MECHALL,
                            "One of your wheels is seriously damaged!");
                break;
              case MOVE_HOVER:
                mech_notify(mech, MECHALL,
                            "Your air skirt is seriously damaged!");
                break;
              case MOVE_HULL:
              case MOVE_SUB:
              case MOVE_FOIL:
                mech_notify(
                    mech, MECHALL,
                    "Your craft lurches and suddenly loses a lot of speed!");
                break;
              }
              mech_max_speed_lower(mech, MP2);
            }
            return side;
          }
          /* Cripple tank */
          if (!Fallen(mech)) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch (MechMove(mech)) {
            case MOVE_TRACK:
              mech_notify(mech, MECHALL,
                          "One of your tracks is destroyed, immobilizing your "
                          "vehicle!");
              break;
            case MOVE_WHEEL:
              mech_notify(mech, MECHALL,
                          "One of your wheels is destroyed, immobilizing your "
                          "vehicle!");
              break;
            case MOVE_HOVER:
              mech_notify(
                  mech, MECHALL,
                  "Your lift fan is destroyed, immobilizing your vehicle!");
              break;
            case MOVE_HULL:
            case MOVE_SUB:
            case MOVE_FOIL:
              mech_notify(mech, MECHALL,
                          "Your engines cut out and you drift to a halt!");
            }
            mech_max_speed_set(mech, 0.0);

            mech_make_fall(mech);
          }
        }
        return side;
      case 4:
        /* MP -1 */
        if (mech->xcode.context->configuration->btech_tankshield) {
          if (!Fallen(mech)) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch (MechMove(mech)) {
            case MOVE_TRACK:
              mech_notify(mech, MECHALL, "One of your tracks is damaged!");
              break;
            case MOVE_WHEEL:
              mech_notify(mech, MECHALL, "One of your wheels is damaged!");
              break;
            case MOVE_HOVER:
              mech_notify(mech, MECHALL, "Your air skirt is damaged!");
              break;
            case MOVE_HULL:
            case MOVE_SUB:
            case MOVE_FOIL:
              mech_notify(mech, MECHALL, "Your craft suddenly slows!");
              break;
            }
            mech_max_speed_lower(mech, MP1);
          }
        }
        return side;
      case 5:
        /* MP -1 if Hovercraft */
        if (!Fallen(mech)) {
          if (MechMove(mech) == MOVE_HOVER) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            mech_notify(mech, MECHALL, "Your air skirt is damaged!");
            mech_max_speed_lower(mech, MP1);
          }
        }
        return side;
      case 6:
      case 7:
      case 8:
      case 9:
        return side;
      case 10:
        return (GetSectInt(mech, TURRET)) ? TURRET : side;
      case 11:
        *iscritical = 1;
        /* Lock turret into place */
        if (GetSectInt(mech, TURRET)) {
          if (!(MechTankCritStatus(mech) & TURRET_LOCKED)) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            MechTankCritStatus(mech) |= TURRET_LOCKED;
            mech_notify(mech, MECHALL,
                        "Your turret takes a direct hit and locks up!");
          }
          return TURRET;
        } else
          return side;
      case 12:
        /* A Roll on Determining Critical Hits Table */
        if (crittable(mech, (GetSectInt(mech, TURRET)) ? TURRET : side,
                      mech->xcode.context->configuration->btech_critlevel))
          *iscritical = 1;
        return (GetSectInt(mech, TURRET)) ? TURRET : side;
      }
    }
    break;
  }
  return hitloc;
}
