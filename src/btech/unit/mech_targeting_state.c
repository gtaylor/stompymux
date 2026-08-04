#include "mech_targeting_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"
#include "section_types.h"

void mech_targeting_lock_modes_clear(Mech *mech) {
  mech->rd.status &= ~LOCK_MODES;
}

void mech_targeting_aim_reset(Mech *mech) { mech->rd.aim = NUM_SECTIONS; }

void mech_targeting_target_clear(Mech *mech) {
  mech->rd.target = -1;
  mech->rd.targx = -1;
  mech->rd.targy = -1;
}

void mech_targeting_tag_clear(Mech *mech) { mech->sd.tagTarget = -1; }

bool mech_targeting_has_lock_on(const Mech *mech, DbRef target) {
  return (mech->rd.status & LOCK_TARGET) && mech->rd.target == target;
}

bool mech_targeting_lock_modes_active(const Mech *mech) {
  return mech->rd.status & LOCK_MODES;
}

bool mech_targeting_has_specific_aim(const Mech *mech) {
  return mech->rd.aim != NUM_SECTIONS;
}

bool mech_movement_modes_locked(const Mech *mech) {
  return mech->rd.status2 & MOVE_MODES_LOCK;
}

bool mech_is_dodging(const Mech *mech) { return mech->rd.status2 & DODGING; }

void mech_digging_clear(Mech *mech) { mech->rd.tankcritstatus &= ~DIGGING_IN; }
