/* Implements BattleTech combat mechanics for unit hitloc fasa. */

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

HitLocationResult mech_fasa_hit_location(Mech *mech, int hitGroup,
                                         HitLocationResult result) {
  int roll;

  result.critical = false;
  BtechContext *context = mech_context(mech);
  roll = btech_random_roll(context);

  MechConditionSummary condition = mech_condition_summary(mech);
  if (condition.combat_safe)
    return hit_location_result_at(result, 0);

  if (condition.dug_in && mech_section_original_internal(mech, TURRET) &&
      btech_random_range(context, 1, 100) >= 42)
    return hit_location_result_at(result, TURRET);

  btech_context_hit_roll_record(context, roll);

  switch (mech_class(mech)) {
  case CLASS_BSUIT:
  case CLASS_MW:
  case CLASS_MECH:
    return fasa_mech_hit_location(mech, hitGroup, result, roll);
  case CLASS_VEH_GROUND:
    return fasa_ground_hit_location(mech, hitGroup, result, roll);
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    return fasa_aerospace_hit_location(mech, hitGroup, result, roll);
  case CLASS_VTOL:
  case CLASS_VEH_NAVAL:
    return fasa_vtol_naval_hit_location(mech, hitGroup, result, roll);
  }
  return hit_location_result_at(result, 0);
}
