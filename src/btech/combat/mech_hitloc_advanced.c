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

static int hit_location_or_fallback(const Mech *mech, int preferred,
                                    int fallback) {
  return mech_section_internal(mech, preferred) > 0 ? preferred : fallback;
}

int mech_advanced_vehicle_hit_location(Mech *mech, int hitGroup,
                                       int *iscritical, int *isrear) {
  int roll, hitloc = 0;
  int side;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);

  *iscritical = 0;
  roll = btech_random_roll(context);

  if (condition.combat_safe)
    return 0;

  if (condition.dug_in && mech_section_internal(mech, TURRET) &&
      btech_random_range(context, 1, 100) >= 42)
    return TURRET;

  btech_context_hit_roll_record(context, roll);

  switch ((int)mech_class(mech)) {
  case CLASS_VEH_GROUND:
    switch (hitGroup) {
    case LEFTSIDE:
    case RIGHTSIDE:
      side = (hitGroup == LEFTSIDE ? LSIDE : RSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
        hitloc = side;
        if (mech_section_is_crittable(mech, hitloc,
                                      btech_context_critical_level(context)))
          mech_motive_system_hit(mech, 0);
        break;
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = FSIDE;
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = BSIDE;
        break;
      case 10:
      case 11:
        hitloc = hit_location_or_fallback(mech, TURRET, side);
        break;
      case 12:
        hitloc = hit_location_or_fallback(mech, TURRET, side);
        *iscritical = 1;
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
        hitloc = side;

        if (mech_section_is_crittable(mech, hitloc,
                                      btech_context_critical_level(context)))
          mech_motive_system_hit(mech, 0);
        break;
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = (hitGroup == FRONT ? RSIDE : LSIDE);
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = (hitGroup == FRONT ? LSIDE : RSIDE);
        break;
      case 10:
      case 11:
        hitloc = hit_location_or_fallback(mech, TURRET,
                                          (hitGroup == FRONT ? LSIDE : RSIDE));
        break;
      case 12:
        hitloc = hit_location_or_fallback(mech, TURRET,
                                          (hitGroup == FRONT ? LSIDE : RSIDE));
        *iscritical = 1;
        break;
      }
      break;
    }
    break;

  case CLASS_VTOL:
    switch (hitGroup) {
    case LEFTSIDE:
    case RIGHTSIDE:
      side = (hitGroup == LEFTSIDE ? LSIDE : RSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = FSIDE;
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = BSIDE;
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        break;
      case 12:
        hitloc = ROTOR;
        *iscritical = 1;
        break;
      }
      break;

    case FRONT:
    case BACK:
      side = (hitGroup == FRONT ? FSIDE : BSIDE);

      switch (roll) {
      case 2:
        hitloc = side;
        *iscritical = 1;
        break;
      case 3:
        hitloc = side;
        break;
      case 4:
        hitloc = side;
        break;
      case 5:
        hitloc = (hitGroup == FRONT ? RSIDE : LSIDE);
        break;
      case 6:
      case 7:
      case 8:
        hitloc = side;
        break;
      case 9:
        hitloc = (hitGroup == FRONT ? LSIDE : RSIDE);
        break;
      case 10:
      case 11:
        hitloc = ROTOR;
        break;
      case 12:
        hitloc = ROTOR;
        *iscritical = 1;
        break;
      }
      break;
    }
    break;
  }

  if (!mech_section_is_crittable(mech, hitloc,
                                 btech_context_critical_level(context)))
    *iscritical = 0;

  return hitloc;
}
