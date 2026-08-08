/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_specification_api.h"

int fasa_ground_hit_location(Mech *mech, int hitGroup, int *iscritical,
                             int *isrear, int roll) {
  int hitloc = 0;
  int side;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);

  switch ((int)mech_class(mech)) {
  case CLASS_VEH_GROUND:
    switch (hitGroup) {

    case LEFTSIDE:
      switch (roll) {
      case 2:
        /* A Roll on Determining Critical Hits Table */
        *iscritical = 1;
        return LSIDE;
      case 3:
        if (btech_context_uses_tank_friendly_criticals(context)) {
          if (!condition.fallen) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch ((int)mech_movement_type(mech)) {
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
        if (!condition.fallen) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch ((int)mech_movement_type(mech)) {
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
        if (!condition.fallen) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch ((int)mech_movement_type(mech)) {
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
        return (mech_section_internal(mech, TURRET)) ? TURRET : LSIDE;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          if (!condition.turret_locked) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            mech_turret_locked_set(mech, true);
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
        if (btech_context_uses_tank_friendly_criticals(context)) {
          if (!condition.fallen) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch ((int)mech_movement_type(mech)) {
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
        if (!condition.fallen) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch ((int)mech_movement_type(mech)) {
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
        if (!condition.fallen) {
          mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
          switch ((int)mech_movement_type(mech)) {
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
        if (!condition.fallen) {
          if (mech_movement_type(mech) == MOVE_HOVER) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            mech_notify(mech, MECHALL, "Your air skirt is damaged!");
            mech_max_speed_lower(mech, MP1);
          }
        }
        return RSIDE;
      case 10:
        return (mech_section_internal(mech, TURRET)) ? TURRET : RSIDE;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          if (!condition.turret_locked) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            mech_turret_locked_set(mech, true);
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
        if (btech_context_uses_tank_critical_shielding(context)) {
          if (btech_context_uses_tank_friendly_criticals(context)) {
            if (!condition.fallen) {
              mech_notify(mech, MECHALL,
                          "[fg=yellow bold]CRITICAL HIT![reset]");
              switch ((int)mech_movement_type(mech)) {
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
          if (!condition.fallen) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch ((int)mech_movement_type(mech)) {
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
        if (btech_context_uses_tank_critical_shielding(context)) {
          if (!condition.fallen) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            switch ((int)mech_movement_type(mech)) {
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
        if (!condition.fallen) {
          if (mech_movement_type(mech) == MOVE_HOVER) {
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
        return (mech_section_internal(mech, TURRET)) ? TURRET : side;
      case 11:
        *iscritical = 1;
        /* Lock turret into place */
        if (mech_section_internal(mech, TURRET)) {
          if (!condition.turret_locked) {
            mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
            mech_turret_locked_set(mech, true);
            mech_notify(mech, MECHALL,
                        "Your turret takes a direct hit and locks up!");
          }
          return TURRET;
        } else
          return side;
      case 12:
        /* A Roll on Determining Critical Hits Table */
        if (mech_section_is_crittable(
                mech, (mech_section_internal(mech, TURRET)) ? TURRET : side,
                btech_context_critical_level(context)))
          *iscritical = 1;
        return (mech_section_internal(mech, TURRET)) ? TURRET : side;
      }
    }
    break;
  }
  return hitloc;
}
