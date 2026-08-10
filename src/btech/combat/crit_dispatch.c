/* Dispatches and resolves unit critical hits. */

#include "btech/context.h"
#include "crit_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include <stddef.h>

void mech_critical_handle(const CriticalHitDispatch *dispatch) {
  Mech *wounded = dispatch->wounded;
  Mech *attacker = dispatch->attacker;
  const int LOS = dispatch->line_of_sight;
  const int hitloc = dispatch->section;
  int num = dispatch->count;
  int i;
  int critHit;
  int critType, critData;
  int count, index;
  int critList[NUM_CRITICALS];
  BtechContext *context = mech_context(wounded);
  MechConditionSummary condition = mech_condition_summary(wounded);

  if (condition.combat_safe)
    return;
  if (mech_technology_flags(wounded) & CRITPROOF_TECH)
    return;
  if (mech_class(wounded) == CLASS_MW && btech_random_range(context, 1, 2) == 1)
    return;
  if (mech_class(wounded) != CLASS_MECH &&
      !btech_context_vehicle_critical_mode(context))
    return;
  if (mech_class(wounded) == CLASS_VEH_GROUND ||
      mech_class(wounded) == CLASS_VEH_NAVAL) {
    if (btech_context_uses_advanced_vehicle_criticals(context)) {
      for (i = 0; i < num; i++)
        mech_advanced_vehicle_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = hitloc});

      return;
    } else if (!btech_context_uses_fasa_criticals(context)) {
      for (i = 0; i < num; i++)
        mech_vehicle_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = hitloc});
      return;
    } else if (btech_context_uses_fasa_criticals(context)) {
      for (i = 0; i < num; i++)
        mech_fasa_vehicle_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = hitloc});
      return;
    }
  }
  if (mech_is_dropship(wounded))
    return;
  if (mech_class(wounded) == CLASS_VTOL) {
    if (btech_context_uses_advanced_vtol_criticals(context)) {
      for (i = 0; i < num; i++)
        mech_advanced_vehicle_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = hitloc});

      return;
    } else {
      for (i = 0; i < num; i++)
        mech_vtol_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = hitloc});

      return;
    }
  }
  while (num > 0) {
    count = 0;
    while (count == 0) {
      for (i = 0; i < NUM_CRITICALS; i++) {
        critType = mech_critical_part_type(wounded, hitloc, i);
        if (!mech_critical_is_destroyed(wounded, hitloc, i) &&
            !mech_critical_is_damaged(wounded, hitloc, i) &&
            critType != EMPTY && critType != special_equipment_index(CASE) &&
            critType != special_equipment_index(FERRO_FIBROUS) &&
            critType != special_equipment_index(STEALTH_ARMOR) &&
            critType != special_equipment_index(HVY_FERRO_FIBROUS) &&
            critType != special_equipment_index(LT_FERRO_FIBROUS) &&
            critType != special_equipment_index(ENDO_STEEL) &&
            critType != special_equipment_index(TRIPLE_STRENGTH_MYOMER) &&
            critType != special_equipment_index(SUPERCHARGER) &&
            critType != special_equipment_index(MASC)) {
          *(int *)checked_storage_at(critList, NUM_CRITICALS, sizeof(*critList),
                                     (size_t)count) = i;
          count++;
        }
      }

      if (!count) /* transfer Crit to next location - no longer */
        return;
    }

    index = btech_random_range_int(context, 0, count - 1);
    critHit = *(const int *)checked_storage_at_const(
        critList, NUM_CRITICALS, sizeof(*critList),
        (size_t)index); /* This one should be linear */

    critType = mech_critical_part_type(wounded, hitloc, critHit);
    critData = mech_critical_data(wounded, hitloc, critHit);

    if (mech_critical_effect_apply(&(CriticalEffectRequest){
            .wounded = wounded,
            .attacker = attacker,
            .line_of_sight = LOS,
            .slot = {.section = hitloc, .critical = critHit},
            .part_type = critType,
            .part_data = critData}))
      num--;
  }
}
