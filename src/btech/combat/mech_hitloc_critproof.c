/* Implements BattleTech combat mechanics for unit hitloc critproof. */

#include "btech/context.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_hitloc_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

/* Use this when the unit is CRITPROOF because the other
 * hitlocation functions are screwy */
int mech_critproof_hit_location(Mech *mech, int hit_group, bool *iscritical) {
  int roll;
  int hitloc = 0;
  int side;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);

  roll = btech_random_roll(context);

  /* Since we're crit proof set this to 0 */
  *iscritical = false;

  if (condition.combat_safe)
    return 0;

  if (condition.dug_in && mech_section_original_internal(mech, TURRET) &&
      btech_random_range(context, 1, 100) >= 42)
    return TURRET;

  btech_context_hit_roll_record(context, roll);
  switch (mech_class(mech)) {
  case CLASS_BSUIT:
    hitloc = mech_battle_suit_hit_location(mech);
    if (hitloc < 0)
      return btech_random_range_int(context, 0, NUM_BSUIT_MEMBERS - 1);
    [[fallthrough]];
  case CLASS_MW:
  case CLASS_MECH:
    switch (hit_group) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
        return LTORSO;
      case 3:
        return LLEG;
      case 4:
      case 5:
        return LARM;
      case 6:
        return LLEG;
      case 7:
        return LTORSO;
      case 8:
        return CTORSO;
      case 9:
        return RTORSO;
      case 10:
        return RARM;
      case 11:
        return RLEG;
      case 12:
        if (btech_context_uses_exile_stun_code(context))
          return mech_head_hit_modify(hit_group, mech);
        return HEAD;
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
        return RTORSO;
      case 3:
        return RLEG;
      case 4:
      case 5:
        return RARM;
      case 6:
        return RLEG;
      case 7:
        return RTORSO;
      case 8:
        return CTORSO;
      case 9:
        return LTORSO;
      case 10:
        return LARM;
      case 11:
        return LLEG;
      case 12:
        if (btech_context_uses_exile_stun_code(context))
          return mech_head_hit_modify(hit_group, mech);
        return HEAD;
      }
      break;
    case FRONT:
    case BACK:
      switch (roll) {
      case 2:
        return CTORSO;
      case 3:
      case 4:
        return RARM;
      case 5:
        return RLEG;
      case 6:
        return RTORSO;
      case 7:
        return CTORSO;
      case 8:
        return LTORSO;
      case 9:
        return LLEG;
      case 10:
      case 11:
        return LARM;
      case 12:
        if (btech_context_uses_exile_stun_code(context))
          return mech_head_hit_modify(hit_group, mech);
        return HEAD;
      }
    }
    break;
  case CLASS_VEH_GROUND:
    switch (hit_group) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
      case 12:
        return LSIDE;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        return LSIDE;
      case 10:
        return (mech_section_internal(mech, TURRET)) ? TURRET : LSIDE;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          return TURRET;
        } else {
          return LSIDE;
        }
      }
      break;
    case RIGHTSIDE:
      switch (roll) {
      case 2:
      case 12:
        return RSIDE;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        return RSIDE;
      case 10:
        return (mech_section_internal(mech, TURRET)) ? TURRET : RSIDE;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          return TURRET;
        } else {
          return RSIDE;
        }
      }
      break;

    case FRONT:
    case BACK:
      side = (hit_group == FRONT ? FSIDE : BSIDE);
      switch (roll) {
      case 2:
        return side;
      case 12:
        return side;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        return side;
      case 10:
        return (mech_section_internal(mech, TURRET)) ? TURRET : side;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          return TURRET;
        } else {
          return side;
        }
      }
    }
    break;
  case CLASS_AERO:
    switch (hit_group) {
    case FRONT:
      switch (roll) {
      case 2:
      case 12:
      case 3:
      case 11:
        return AERO_NOSE;
      case 4:
      case 10:
      case 5:
        return AERO_RWING;
      case 9:
        return AERO_LWING;
      case 6:
      case 7:
      case 8:
        return AERO_NOSE;
      }
      break;
    case LEFTSIDE:
    case RIGHTSIDE:
      side = ((hit_group == LEFTSIDE) ? AERO_LWING : AERO_RWING);
      switch (roll) {
      case 2:
      case 12:
        return AERO_AFT;
      case 3:
      case 11:
        return side;
      case 4:
      case 10:
        return AERO_AFT;
      case 5:
      case 9:
        return AERO_NOSE;
      case 6:
      case 8:
        return side;
      case 7:
        return side;
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
      case 12:
        return AERO_AFT;
      case 3:
      case 11:
      case 4:
      case 7:
      case 10:
      case 5:
        return AERO_RWING;
      case 9:
        return AERO_LWING;
      case 6:
      case 8:
        return AERO_AFT;
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
        return DS_NOSE;
      case 3:
      case 11:
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
      side = (hit_group == LEFTSIDE) ? DS_LWING : DS_RWING;
      if (btech_random_range(context, 1, 2) == 2)
        side = mech_spheroid_rear_section(mech, side);
      switch (roll) {
      case 2:
        return DS_NOSE;
      case 3:
      case 11:
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
        return side;
      }
      break;
    case BACK:
      switch (roll) {
      case 2:
      case 12:
        return DS_AFT;
      case 3:
      case 11:
        return DS_AFT;
      case 4:
      case 7:
      case 10:
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
  case CLASS_VTOL:
    switch (hit_group) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
        hitloc = ROTOR;
        break;
      case 3:
      case 4:
        hitloc = ROTOR;
        break;
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = LSIDE;
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        break;
      case 12:
        hitloc = ROTOR;
        break;
      }
      break;

    case RIGHTSIDE:
      switch (roll) {
      case 2:
        hitloc = ROTOR;
        break;
      case 3:
      case 4:
        hitloc = ROTOR;
        break;
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = RSIDE;
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        break;
      case 12:
        hitloc = ROTOR;
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hit_group == FRONT ? FSIDE : BSIDE);
      switch (roll) {
      case 2:
        hitloc = ROTOR;
        break;
      case 3:
      case 4:
        hitloc = ROTOR;
        break;
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = side;
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        break;
      case 12:
        hitloc = ROTOR;
        break;
      }
      break;
    }
    break;
  case CLASS_VEH_NAVAL:
    switch (hit_group) {
    case LEFTSIDE:
      switch (roll) {
      case 2:
        hitloc = LSIDE;
        break;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
        hitloc = LSIDE;
        break;
      case 9:
        hitloc = LSIDE;
        break;
      case 10:
        if (mech_section_internal(mech, TURRET))
          hitloc = TURRET;
        else
          hitloc = LSIDE;
        break;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          hitloc = TURRET;
        } else {
          hitloc = LSIDE;
        }
        break;
      case 12:
        hitloc = LSIDE;
        break;
      }
      break;

    case RIGHTSIDE:
      switch (roll) {
      case 2:
      case 12:
        hitloc = RSIDE;
        break;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = RSIDE;
        break;
      case 10:
        if (mech_section_internal(mech, TURRET))
          hitloc = TURRET;
        else
          hitloc = RSIDE;
        break;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          hitloc = TURRET;
        } else {
          hitloc = RSIDE;
        }
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hit_group == FRONT ? FSIDE : BSIDE);
      switch (roll) {
      case 2:
        hitloc = side;
        break;
      case 12:
        hitloc = side;
        break;
      case 3:
      case 4:
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        hitloc = side;
        break;
      case 10:
        if (mech_section_internal(mech, TURRET))
          hitloc = TURRET;
        else
          hitloc = side;
        break;
      case 11:
        if (mech_section_internal(mech, TURRET)) {
          hitloc = TURRET;
        } else {
          hitloc = side;
        }
        break;
      }
      break;
    }
    break;
  }
  return (hitloc);
}
