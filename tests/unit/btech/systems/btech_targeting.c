#include "mech_targeting_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

static Mech *next_unit(Mech *unit, int *evaluations) {
  ++*evaluations;
  return unit;
}

int main(void) {
  Mech mech = {};
  MechTargetingOverride override;
  DbRef target;
  int target_x;
  int target_y;
  int target_z;
  int lock_modes;
  int evaluations = 0;

  mech_target_dbref_set(next_unit(&mech, &evaluations), 7);
  if (evaluations != 1 || mech.rd.target != 7)
    return 1;
  mech.rd.status = LOCK_HEX;
  mech.rd.targx = 8;
  mech_target_dbref_set(&mech, 9);
  if (mech.rd.status != LOCK_HEX || mech.rd.targx != 8)
    return 2;

  mech_targeting_aim_set(
      &mech, (MechAimSelection){.section = HEAD, .unit_class = CLASS_MECH});
  mech_targeting_aim_reset(&mech);
  mech_charge_target_dbref_set(&mech, 10);
  mech_dfa_target_dbref_set(&mech, 11);
  mech_spotter_dbref_set(&mech, 12);
  mech_targeting_computer_type_set(&mech, TARGCOMP_AA);
  if (mech_aim_section(&mech) != NUM_SECTIONS ||
      mech_charge_target_dbref(&mech) != 10 ||
      mech_dfa_target_dbref(&mech) != 11 || mech_spotter_dbref(&mech) != 12 ||
      mech_targeting_computer_type(&mech) != TARGCOMP_AA)
    return 3;

  mech.rd.status = STARTED | LOCK_TARGET;
  mech.rd.target = 11;
  mech.rd.targx = 12;
  mech.rd.targy = 13;
  mech.rd.targz = 14;

  mech_targeting_override_begin(
      &(MechTargetingOverrideBegin){.mech = &mech,
                                    .override = &override,
                                    .state = {.target = 21,
                                              .target_x = 22,
                                              .target_y = 23,
                                              .target_z = 24,
                                              .lock_modes = LOCK_HEX}});
  if (mech.rd.status != (STARTED | LOCK_HEX) || mech.rd.target != 21 ||
      mech.rd.targx != 22 || mech.rd.targy != 23 || mech.rd.targz != 24)
    return 4;

  mech.rd.status = FIRED | LOCK_BUILDING;
  mech.rd.target = 31;
  mech.rd.targx = 32;
  mech.rd.targy = 33;
  mech.rd.targz = 34;
  MechTargetingState state = mech_targeting_override_end(&mech, &override);
  target = state.target;
  target_x = state.target_x;
  target_y = state.target_y;
  target_z = state.target_z;
  lock_modes = state.lock_modes;

  if (target != 31 || target_x != 32 || target_y != 33 || target_z != 34 ||
      lock_modes != LOCK_BUILDING)
    return 5;
  if (mech.rd.status != (STARTED | LOCK_TARGET) || mech.rd.target != 11 ||
      mech.rd.targx != 12 || mech.rd.targy != 13 || mech.rd.targz != 14)
    return 6;

  mech.rd.chgtarget = 41;
  mech.rd.chargetimer = 42;
  mech.rd.chargedist = 43.0F;
  mech_charge_reset(&mech);
  if (mech.rd.chgtarget != -1 || mech.rd.chargetimer != 0 ||
      mech.rd.chargedist != 0.0F)
    return 7;
  return 0;
}
