#include "mech_targeting_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

int main(void) {
  Mech mech = {0};
  MechTargetingOverride override;
  DbRef target;
  int target_x;
  int target_y;
  int target_z;
  int lock_modes;

  mech.rd.status = STARTED | LOCK_TARGET;
  mech.rd.target = 11;
  mech.rd.targx = 12;
  mech.rd.targy = 13;
  mech.rd.targz = 14;

  mech_targeting_override_begin(&mech, &override, 21, 22, 23, 24, LOCK_HEX);
  if (mech.rd.status != (STARTED | LOCK_HEX) || mech.rd.target != 21 ||
      mech.rd.targx != 22 || mech.rd.targy != 23 || mech.rd.targz != 24)
    return 1;

  mech.rd.status = FIRED | LOCK_BUILDING;
  mech.rd.target = 31;
  mech.rd.targx = 32;
  mech.rd.targy = 33;
  mech.rd.targz = 34;
  mech_targeting_override_end(&mech, &override, &target, &target_x, &target_y,
                              &target_z, &lock_modes);

  if (target != 31 || target_x != 32 || target_y != 33 || target_z != 34 ||
      lock_modes != LOCK_BUILDING)
    return 2;
  if (mech.rd.status != (STARTED | LOCK_TARGET) || mech.rd.target != 11 ||
      mech.rd.targx != 12 || mech.rd.targy != 13 || mech.rd.targz != 14)
    return 3;
  return 0;
}
