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

int mech_fasa_hit_location(Mech *mech, int hitGroup, int *iscritical,
                           int *isrear) {
  int roll;

  *iscritical = 0;
  BtechContext *context = mech_context(mech);
  roll = btech_random_roll(context);

  MechConditionSummary condition = mech_condition_summary(mech);
  if (condition.combat_safe)
    return 0;

  if (condition.dug_in && mech_section_original_internal(mech, TURRET) &&
      btech_random_range(context, 1, 100) >= 42)
    return TURRET;

  btech_context_hit_roll_record(context, roll);

  switch ((int)mech_class(mech)) {
  case CLASS_BSUIT:
  case CLASS_MW:
  case CLASS_MECH:
    return fasa_mech_hit_location(mech, hitGroup, iscritical, isrear, roll);
  case CLASS_VEH_GROUND:
    return fasa_ground_hit_location(mech, hitGroup, iscritical, isrear, roll);
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    return fasa_aerospace_hit_location(mech, hitGroup, iscritical, isrear,
                                       roll);
  case CLASS_VTOL:
  case CLASS_VEH_NAVAL:
    return fasa_vtol_naval_hit_location(mech, hitGroup, iscritical, isrear,
                                        roll);
  }
  return 0;
}
