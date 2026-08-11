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
  const int HITLOC = dispatch->section;
  int num = dispatch->count;
  int i;
  int crit_hit;
  int crit_type, crit_data;
  int count, index;
  int crit_list[NUM_CRITICALS];
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
                                      .section = HITLOC});

      return;
    } else if (!btech_context_uses_fasa_criticals(context)) {
      for (i = 0; i < num; i++)
        mech_vehicle_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = HITLOC});
      return;
    } else if (btech_context_uses_fasa_criticals(context)) {
      for (i = 0; i < num; i++)
        mech_fasa_vehicle_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = HITLOC});
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
                                      .section = HITLOC});

      return;
    } else {
      for (i = 0; i < num; i++)
        mech_vtol_critical_handle(
            &(VehicleCriticalRequest){.wounded = wounded,
                                      .attacker = attacker,
                                      .line_of_sight = LOS,
                                      .section = HITLOC});

      return;
    }
  }
  while (num > 0) {
    count = 0;
    while (count == 0) {
      for (i = 0; i < NUM_CRITICALS; i++) {
        crit_type = mech_critical_part_type(wounded, HITLOC, i);
        if (!mech_critical_is_destroyed(wounded, HITLOC, i) &&
            !mech_critical_is_damaged(wounded, HITLOC, i) &&
            crit_type != EMPTY && crit_type != special_equipment_index(CASE) &&
            crit_type != special_equipment_index(FERRO_FIBROUS) &&
            crit_type != special_equipment_index(STEALTH_ARMOR) &&
            crit_type != special_equipment_index(HVY_FERRO_FIBROUS) &&
            crit_type != special_equipment_index(LT_FERRO_FIBROUS) &&
            crit_type != special_equipment_index(ENDO_STEEL) &&
            crit_type != special_equipment_index(TRIPLE_STRENGTH_MYOMER) &&
            crit_type != special_equipment_index(SUPERCHARGER) &&
            crit_type != special_equipment_index(MASC)) {
          *(int *)checked_storage_at(crit_list, NUM_CRITICALS,
                                     sizeof(*crit_list), (size_t)count) = i;
          count++;
        }
      }

      if (!count) /* transfer Crit to next location - no longer */
        return;
    }

    index = btech_random_range_int(context, 0, count - 1);
    crit_hit = *(const int *)checked_storage_at_const(
        crit_list, NUM_CRITICALS, sizeof(*crit_list),
        (size_t)index); /* This one should be linear */

    crit_type = mech_critical_part_type(wounded, HITLOC, crit_hit);
    crit_data = mech_critical_data(wounded, HITLOC, crit_hit);

    if (mech_critical_effect_apply(&(CriticalEffectRequest){
            .wounded = wounded,
            .attacker = attacker,
            .line_of_sight = LOS,
            .slot = {.section = HITLOC, .critical = crit_hit},
            .part_type = crit_type,
            .part_data = crit_data}))
      num--;
  }
}
