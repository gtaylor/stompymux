#include "mech_targeting_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"
#include "section_types.h"

#include <string.h>

typedef struct MechTargetingOverrideStorage {
  DbRef target;
  int target_x;
  int target_y;
  int target_z;
  int status;
} MechTargetingOverrideStorage;

static_assert(sizeof(MechTargetingOverrideStorage) <=
              sizeof(MechTargetingOverride));

void mech_targeting_lock_modes_clear(Mech *mech) {
  mech->rd.status &= ~LOCK_MODES;
}

void mech_targeting_aim_reset(Mech *mech) { mech->rd.aim = NUM_SECTIONS; }

void mech_targeting_target_clear(Mech *mech) {
  mech->rd.target = -1;
  mech->rd.targx = -1;
  mech->rd.targy = -1;
}

DbRef mech_target_dbref(const Mech *mech) { return mech->rd.target; }

DbRef mech_charge_target_dbref(const Mech *mech) { return mech->rd.chgtarget; }

DbRef mech_dfa_target_dbref(const Mech *mech) { return mech->rd.dfatarget; }

int mech_charge_timer(const Mech *mech) { return mech->rd.chargetimer; }

int mech_target_hex_x(const Mech *mech) { return mech->rd.targx; }

int mech_target_hex_y(const Mech *mech) { return mech->rd.targy; }

int mech_target_hex_z(const Mech *mech) { return mech->rd.targz; }

void mech_target_hex_z_set(Mech *mech, int z) { mech->rd.targz = z; }

DbRef mech_spotter_dbref(const Mech *mech) { return mech->rd.spotter; }

void mech_spotter_dbref_set(Mech *mech, DbRef spotter) {
  mech->rd.spotter = spotter;
}

void mech_fire_adjustment_set(Mech *mech, int adjustment) {
  mech->rd.fire_adjustment = adjustment;
}

int mech_targeting_computer_type(const Mech *mech) { return mech->ud.targcomp; }

int mech_aim_section(const Mech *mech) { return mech->rd.aim; }

int mech_aim_unit_class(const Mech *mech) { return mech->rd.aim_type; }

bool mech_targets_building(const Mech *mech) {
  return mech->rd.status & LOCK_BUILDING;
}

bool mech_targets_hex(const Mech *mech) { return mech->rd.status & LOCK_HEX; }

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

void mech_targeting_override_begin(Mech *mech, MechTargetingOverride *override,
                                   DbRef target, int target_x, int target_y,
                                   int target_z, int lock_modes) {
  const MechTargetingOverrideStorage storage = {
      .target = mech->rd.target,
      .target_x = mech->rd.targx,
      .target_y = mech->rd.targy,
      .target_z = mech->rd.targz,
      .status = mech->rd.status,
  };
  *override = (MechTargetingOverride){0};
  memcpy(override, &storage, sizeof(storage));
  mech->rd.status = (mech->rd.status & ~LOCK_MODES) | lock_modes;
  mech->rd.target = target;
  mech->rd.targx = target_x;
  mech->rd.targy = target_y;
  mech->rd.targz = target_z;
}

void mech_targeting_override_end(Mech *mech,
                                 const MechTargetingOverride *override,
                                 DbRef *target, int *target_x, int *target_y,
                                 int *target_z, int *lock_modes) {
  MechTargetingOverrideStorage storage;

  memcpy(&storage, override, sizeof(storage));
  *target = mech->rd.target;
  *target_x = mech->rd.targx;
  *target_y = mech->rd.targy;
  *target_z = mech->rd.targz;
  *lock_modes = mech->rd.status & LOCK_MODES;
  mech->rd.status = storage.status;
  mech->rd.target = storage.target;
  mech->rd.targx = storage.target_x;
  mech->rd.targy = storage.target_y;
  mech->rd.targz = storage.target_z;
}
