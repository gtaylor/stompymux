/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_classification_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"

int fasa_aerospace_hit_location(Mech *mech, int hitGroup, int *iscritical,
                                int *isrear, int roll) {
  int hitloc = 0;
  int side;
  BtechContext *context = mech_context(mech);

  switch (mech_class(mech)) {
  case CLASS_AERO:
    switch (hitGroup) {
    case FRONT:
      switch (roll) {
      case 2:
        // Nose/Weapons
        return AERO_NOSE;
      case 3:
        // Nose/Sensors
        return AERO_NOSE;
      case 4:
        // Right Wing/Heat Sink
        return AERO_RWING;
      case 5:
        // Right Wing/Weapon
        return AERO_RWING;
      case 6:
        // Nose/Avionics
        return AERO_NOSE;
      case 7:
        // Nose/Control
        return AERO_NOSE;
      case 8:
        // Nose/FCS
        return AERO_NOSE;
      case 9:
        // Left Wing/Weapon
        return AERO_LWING;
      case 10:
        // Left Wing/Heat Sink
        return AERO_LWING;
      case 11:
        // Nose/Gear
        if (mech_section_is_crittable(mech, AERO_NOSE, 90))
          LoseWeapon(mech, AERO_NOSE);
        return AERO_NOSE;
      case 12:
        // Nose/Weapon
        return AERO_NOSE;
      }
      break;
    case LEFTSIDE:
      switch (roll) {
      case 2:
        // Nose/Weapon
        return AERO_NOSE;
      case 3:
        // Wing/Gear
        return AERO_LWING;
      case 4:
        // Nose/Senors
        return AERO_NOSE;
      case 5:
        // Nose/Crew
        return AERO_NOSE;
      case 6:
        // Wing/Weapons
        return AERO_LWING;
      case 7:
        // Wing/Avionics
        return AERO_LWING;
      case 8:
        // Wing/Bomb
        return AERO_LWING;
      case 9:
        // Aft/Control
        return AERO_AFT;
      case 10:
        // Aft/Engine
        return AERO_AFT;
      case 11:
        // Wing/Gear
        return AERO_LWING;
      case 12:
        // Aft/Weapon
        return AERO_AFT;
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
        // Nose/Weapon
        return AERO_NOSE;
      case 3:
        // Wing/Gear
        return AERO_RWING;
      case 4:
        // Nose/Sensors
        return AERO_NOSE;
      case 5:
        // Nose/Crew
        return AERO_NOSE;
      case 6:
        // Wing/Weapons
        return AERO_RWING;
      case 7:
        // Wing/Avionics
        return AERO_RWING;
      case 8:
        // Wing/Bomb
        return AERO_RWING;
      case 9:
        // Aft/Control
        return AERO_AFT;
      case 10:
        // Aft/Engine
        return AERO_AFT;
      case 11:
        // Wing/Gear
        return AERO_RWING;
      case 12:
        // Aft/Weapon
        return AERO_AFT;
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
        // Aft/Weapon
        return AERO_AFT;
      case 3:
        // Aft/Heat Sink
        return AERO_AFT;
      case 4:
        // Right Wing/Fuel
        return AERO_RWING;
      case 5:
        // Right Wing/Weapon
        return AERO_RWING;
      case 6:
        // Aft/Engine
        return AERO_AFT;
      case 7:
        // Aft/Control
        return AERO_AFT;
      case 8:
        // Aft/Engine
        return AERO_AFT;
      case 9:
        // Left Wing/Weapon
        return AERO_LWING;
      case 10:
        // Left Wing/Fuel
        return AERO_LWING;
      case 11:
        // Aft/Heat Sink
        return AERO_AFT;
      case 12:
        // Aft/Weapon
        return AERO_AFT;
      }
    }
    break;
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    switch (hitGroup) {
    case FRONT:
      switch (roll) {
      case 2:
      case 12:
        if (mech_section_is_crittable(mech, DS_NOSE, 30))
          dropship_bridge_hit(mech);
        return DS_NOSE;
      case 3:
      case 11:
        if (mech_section_is_crittable(mech, DS_NOSE, 50))
          LoseWeapon(mech, DS_NOSE);
        return DS_NOSE;
      case 5:
        return DS_RWING;
      case 6:
      case 7:
      case 8:
        return DS_NOSE;
      case 9:
        return DS_LWING;
      case 4:
      case 10:
        return (btech_random_range(context, 1, 2)) == 1 ? DS_LWING : DS_RWING;
      }
      break;
    case LEFTSIDE:
    case RIGHTSIDE:
      side = (hitGroup == LEFTSIDE) ? DS_LWING : DS_RWING;
      if (btech_random_range(context, 1, 2) == 2)
        side = mech_spheroid_rear_section(mech, side);
      switch (roll) {
      case 2:
        if (mech_section_is_crittable(mech, DS_NOSE, 30))
          dropship_bridge_hit(mech);
        return DS_NOSE;
      case 3:
      case 11:
        if (mech_section_is_crittable(mech, side, 60))
          LoseWeapon(mech, side);
        return side;
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 10:
        return side;
      case 9:
        return DS_NOSE;
      case 12:
        if (mech_section_is_crittable(mech, side, 60))
          *iscritical = 1;
        return side;
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
      case 12:
        if (mech_section_is_crittable(mech, DS_AFT, 60))
          *iscritical = 1;
        return DS_AFT;
      case 3:
      case 11:
        return DS_AFT;
      case 4:
      case 7:
      case 10:
        if (mech_section_is_crittable(mech, DS_AFT, 60))
          DestroyHeatSink(mech, DS_AFT);
        return DS_AFT;
      case 5:
        hitloc = DS_RWING;
        hitloc = mech_spheroid_rear_section(mech, hitloc);
        return hitloc;
      case 6:
      case 8:
        return DS_AFT;
      case 9:
        hitloc = DS_LWING;
        hitloc = mech_spheroid_rear_section(mech, hitloc);
        return hitloc;
      }
    }
    break;
  }
  return hitloc;
}
