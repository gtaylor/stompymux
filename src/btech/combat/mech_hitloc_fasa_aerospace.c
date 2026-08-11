/* Implements BattleTech combat mechanics for unit hitloc fasa aerospace. */

#include "aero_move_api.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_damage_api.h"
#include "mech_hitloc_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

HitLocationResult fasa_aerospace_hit_location(Mech *mech, int hit_group,
                                              HitLocationResult result,
                                              int roll) {
  int hitloc = 0;
  int side;
  BtechContext *context = mech_context(mech);

  switch (mech_class(mech)) {
  case CLASS_AERO:
    switch (hit_group) {
    case FRONT:
      switch (roll) {
      case 2:
        // Nose/Weapons
        return hit_location_result_at(result, AERO_NOSE);
      case 3:
        // Nose/Sensors
        return hit_location_result_at(result, AERO_NOSE);
      case 4:
        // Right Wing/Heat Sink
        return hit_location_result_at(result, AERO_RWING);
      case 5:
        // Right Wing/Weapon
        return hit_location_result_at(result, AERO_RWING);
      case 6:
        // Nose/Avionics
        return hit_location_result_at(result, AERO_NOSE);
      case 7:
        // Nose/Control
        return hit_location_result_at(result, AERO_NOSE);
      case 8:
        // Nose/FCS
        return hit_location_result_at(result, AERO_NOSE);
      case 9:
        // Left Wing/Weapon
        return hit_location_result_at(result, AERO_LWING);
      case 10:
        // Left Wing/Heat Sink
        return hit_location_result_at(result, AERO_LWING);
      case 11:
        // Nose/Gear
        if (mech_section_is_crittable(mech, AERO_NOSE, (CriticalThreshold){90}))
          mech_weapon_destroy_random(mech, AERO_NOSE);
        return hit_location_result_at(result, AERO_NOSE);
      case 12:
        // Nose/Weapon
        return hit_location_result_at(result, AERO_NOSE);
      }
      break;
    case LEFTSIDE:
      switch (roll) {
      case 2:
        // Nose/Weapon
        return hit_location_result_at(result, AERO_NOSE);
      case 3:
        // Wing/Gear
        return hit_location_result_at(result, AERO_LWING);
      case 4:
        // Nose/Senors
        return hit_location_result_at(result, AERO_NOSE);
      case 5:
        // Nose/Crew
        return hit_location_result_at(result, AERO_NOSE);
      case 6:
        // Wing/Weapons
        return hit_location_result_at(result, AERO_LWING);
      case 7:
        // Wing/Avionics
        return hit_location_result_at(result, AERO_LWING);
      case 8:
        // Wing/Bomb
        return hit_location_result_at(result, AERO_LWING);
      case 9:
        // Aft/Control
        return hit_location_result_at(result, AERO_AFT);
      case 10:
        // Aft/Engine
        return hit_location_result_at(result, AERO_AFT);
      case 11:
        // Wing/Gear
        return hit_location_result_at(result, AERO_LWING);
      case 12:
        // Aft/Weapon
        return hit_location_result_at(result, AERO_AFT);
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
        // Nose/Weapon
        return hit_location_result_at(result, AERO_NOSE);
      case 3:
        // Wing/Gear
        return hit_location_result_at(result, AERO_RWING);
      case 4:
        // Nose/Sensors
        return hit_location_result_at(result, AERO_NOSE);
      case 5:
        // Nose/Crew
        return hit_location_result_at(result, AERO_NOSE);
      case 6:
        // Wing/Weapons
        return hit_location_result_at(result, AERO_RWING);
      case 7:
        // Wing/Avionics
        return hit_location_result_at(result, AERO_RWING);
      case 8:
        // Wing/Bomb
        return hit_location_result_at(result, AERO_RWING);
      case 9:
        // Aft/Control
        return hit_location_result_at(result, AERO_AFT);
      case 10:
        // Aft/Engine
        return hit_location_result_at(result, AERO_AFT);
      case 11:
        // Wing/Gear
        return hit_location_result_at(result, AERO_RWING);
      case 12:
        // Aft/Weapon
        return hit_location_result_at(result, AERO_AFT);
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
        // Aft/Weapon
        return hit_location_result_at(result, AERO_AFT);
      case 3:
        // Aft/Heat Sink
        return hit_location_result_at(result, AERO_AFT);
      case 4:
        // Right Wing/Fuel
        return hit_location_result_at(result, AERO_RWING);
      case 5:
        // Right Wing/Weapon
        return hit_location_result_at(result, AERO_RWING);
      case 6:
        // Aft/Engine
        return hit_location_result_at(result, AERO_AFT);
      case 7:
        // Aft/Control
        return hit_location_result_at(result, AERO_AFT);
      case 8:
        // Aft/Engine
        return hit_location_result_at(result, AERO_AFT);
      case 9:
        // Left Wing/Weapon
        return hit_location_result_at(result, AERO_LWING);
      case 10:
        // Left Wing/Fuel
        return hit_location_result_at(result, AERO_LWING);
      case 11:
        // Aft/Heat Sink
        return hit_location_result_at(result, AERO_AFT);
      case 12:
        // Aft/Weapon
        return hit_location_result_at(result, AERO_AFT);
      }
    }
    break;
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    switch (hit_group) {
    case FRONT:
      switch (roll) {
      case 2:
      case 12:
        if (mech_section_is_crittable(mech, DS_NOSE, (CriticalThreshold){30}))
          dropship_bridge_hit(mech);
        return hit_location_result_at(result, DS_NOSE);
      case 3:
      case 11:
        if (mech_section_is_crittable(mech, DS_NOSE, (CriticalThreshold){50}))
          mech_weapon_destroy_random(mech, DS_NOSE);
        return hit_location_result_at(result, DS_NOSE);
      case 5:
        return hit_location_result_at(result, DS_RWING);
      case 6:
      case 7:
      case 8:
        return hit_location_result_at(result, DS_NOSE);
      case 9:
        return hit_location_result_at(result, DS_LWING);
      case 4:
      case 10:
        return hit_location_result_at(
            result,
            (btech_random_range(context, 1, 2)) == 1 ? DS_LWING : DS_RWING);
      }
      break;
    case LEFTSIDE:
    case RIGHTSIDE:
      side = (hit_group == LEFTSIDE) ? DS_LWING : DS_RWING;
      if (btech_random_range(context, 1, 2) == 2)
        side = mech_spheroid_rear_section(mech, side);
      switch (roll) {
      case 2:
        if (mech_section_is_crittable(mech, DS_NOSE, (CriticalThreshold){30}))
          dropship_bridge_hit(mech);
        return hit_location_result_at(result, DS_NOSE);
      case 3:
      case 11:
        if (mech_section_is_crittable(mech, side, (CriticalThreshold){60}))
          mech_weapon_destroy_random(mech, side);
        return hit_location_result_at(result, side);
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 10:
        return hit_location_result_at(result, side);
      case 9:
        return hit_location_result_at(result, DS_NOSE);
      case 12:
        if (mech_section_is_crittable(mech, side, (CriticalThreshold){60}))
          result.critical = 1;
        return hit_location_result_at(result, side);
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
      case 12:
        if (mech_section_is_crittable(mech, DS_AFT, (CriticalThreshold){60}))
          result.critical = 1;
        return hit_location_result_at(result, DS_AFT);
      case 3:
      case 11:
        return hit_location_result_at(result, DS_AFT);
      case 4:
      case 7:
      case 10:
        if (mech_section_is_crittable(mech, DS_AFT, (CriticalThreshold){60}))
          mech_heat_sink_destroy(mech, DS_AFT);
        return hit_location_result_at(result, DS_AFT);
      case 5:
        hitloc = DS_RWING;
        hitloc = mech_spheroid_rear_section(mech, hitloc);
        return hit_location_result_at(result, hitloc);
      case 6:
      case 8:
        return hit_location_result_at(result, DS_AFT);
      case 9:
        hitloc = DS_LWING;
        hitloc = mech_spheroid_rear_section(mech, hitloc);
        return hit_location_result_at(result, hitloc);
      }
    }
    break;
  case CLASS_MECH:
  case CLASS_VEH_GROUND:
  case CLASS_VTOL:
  case CLASS_VEH_NAVAL:
  case CLASS_MW:
  case CLASS_BSUIT:
    break;
  }
  return hit_location_result_at(result, hitloc);
}
