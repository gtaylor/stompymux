#pragma once

#include "mech_api_types.h"
#include "section_types.h"

#include <stdbool.h>
#include <stdint.h>

typedef struct MechTargetingOverride {
  uint64_t private_storage[3];
} MechTargetingOverride;

typedef enum TargetingComputerType : int {
  TARGCOMP_NORMAL = 0,
  TARGCOMP_SHORT = 1,
  TARGCOMP_LONG = 2,
  TARGCOMP_MULTI = 3,
  TARGCOMP_AA = 4,
} TargetingComputerType;

void mech_targeting_lock_modes_clear(Mech *mech);
void mech_targeting_aim_reset(Mech *mech);
typedef struct MechAimSelection {
  int section;
  UnitClass unit_class;
} MechAimSelection;

void mech_targeting_aim_set(Mech *mech, MechAimSelection selection);
void mech_targeting_target_clear(Mech *mech);
void mech_targeting_unit_set(Mech *mech, DbRef target);
void mech_targeting_hex_xy_set(Mech *mech, int x, int y);
void mech_targeting_lock_mode_add(Mech *mech, int lock_mode);
DbRef mech_target_dbref(const Mech *mech);
void mech_target_dbref_set(Mech *mech, DbRef target);
DbRef mech_charge_target_dbref(const Mech *mech);
void mech_charge_target_dbref_set(Mech *mech, DbRef target);
DbRef mech_dfa_target_dbref(const Mech *mech);
void mech_dfa_target_dbref_set(Mech *mech, DbRef target);
int mech_charge_timer(const Mech *mech);
int mech_charge_timer_advance(Mech *mech);
float mech_charge_distance(const Mech *mech);
void mech_charge_distance_add(Mech *mech, float distance);
int mech_target_hex_x(const Mech *mech);
int mech_target_hex_y(const Mech *mech);
int mech_target_hex_z(const Mech *mech);
void mech_target_hex_z_set(Mech *mech, int z);
DbRef mech_spotter_dbref(const Mech *mech);
void mech_spotter_dbref_set(Mech *mech, DbRef spotter);
void mech_fire_adjustment_set(Mech *mech, int adjustment);
void mech_fire_adjustment_increment(Mech *mech);
int mech_fire_adjustment(const Mech *mech);
TargetingComputerType mech_targeting_computer_type(const Mech *mech);
void mech_targeting_computer_type_set(Mech *mech, TargetingComputerType type);
int mech_aim_section(const Mech *mech);
UnitClass mech_aim_unit_class(const Mech *mech);
bool mech_targets_building(const Mech *mech);
bool mech_targets_hex(const Mech *mech);
bool mech_targets_hex_for_ignition(const Mech *mech);
bool mech_targets_hex_for_clearing(const Mech *mech);
bool mech_targets_hex_or_building(const Mech *mech);
void mech_targeting_tag_clear(Mech *mech);
bool mech_targeting_has_lock_on(const Mech *mech, DbRef target);
bool mech_targeting_lock_modes_active(const Mech *mech);
bool mech_targeting_has_specific_aim(const Mech *mech);
bool mech_movement_modes_locked(const Mech *mech);
bool mech_is_dodging(const Mech *mech);
void mech_digging_clear(Mech *mech);
typedef struct MechTargetingState {
  DbRef target;
  int target_x;
  int target_y;
  int target_z;
  int lock_modes;
} MechTargetingState;

typedef struct MechTargetingOverrideBegin {
  Mech *mech;
  MechTargetingOverride *override;
  MechTargetingState state;
} MechTargetingOverrideBegin;

void mech_targeting_override_begin(const MechTargetingOverrideBegin *request);
MechTargetingState
mech_targeting_override_end(Mech *mech, const MechTargetingOverride *override);
void mech_charge_reset(Mech *mech);
