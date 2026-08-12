#include "equipment_types.h"
#include "mech_condition_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"
#include "mux/server/platform.h"
#include "section_types.h"

MechConditionSummary mech_condition_summary(const Mech *mech) {
  const int STATUS = mech->rd.status;
  const int STATUS2 = mech->rd.status2;
  const int CRITICAL_STATUS = mech->rd.critstatus;
  const int TANK_CRITICAL_STATUS = mech->rd.tankcritstatus;
  const bool ECM_IS_COUNTERED = STATUS2 & ECM_COUNTERED;
  const bool ECM_ACTIVE = (STATUS2 & ECM_ENABLED) && !ECM_IS_COUNTERED;
  const bool PERSONAL_ECM_ACTIVE =
      (STATUS2 & PER_ECM_ENABLED) && !ECM_IS_COUNTERED;
  const bool ANGEL_ECM_ACTIVE =
      (STATUS2 & ANGEL_ECM_ENABLED) && !ECM_IS_COUNTERED;

  return (MechConditionSummary){
      .combat_safe = STATUS & COMBAT_SAFE,
      .partial_cover = STATUS & PARTIAL_COVER,
      .fallen = STATUS & FALLEN,
      .fortified = STATUS2 & FORTIFIED,
      .weapons_hold = STATUS2 & WEAPONS_HOLD,
      .hull_down = STATUS & HULLDOWN,
      .dug_in = TANK_CRITICAL_STATUS & DUG_IN,
      .digging = TANK_CRITICAL_STATUS & DIGGING_IN,
      .staggering = mech->rd.stagger_damage / 20 > 0,
      .searchlight_destroyed = CRITICAL_STATUS & SLITE_DEST,
      .searchlight_on = STATUS2 & SLITE_ON,
      .illuminated = CRITICAL_STATUS & SLITE_LIT,
      .hidden = CRITICAL_STATUS & HIDDEN,
      .dodging = STATUS2 & DODGING,
      .evading = STATUS2 & EVADING,
      .sprinting = STATUS2 & SPRINTING,
      .stunned = (TANK_CRITICAL_STATUS & CREW_STUNNED) ||
                 (CRITICAL_STATUS & MECH_STUNNED),
      .performing_action = STATUS & PERFORMING_ACTION,
      .auto_fall = mech->rd.mech_prefs & MECHPREF_AUTOFALL,
      .to_hit_debug = mech->rd.mech_prefs & MECHPREF_BTHDEBUG,
      .ecm_disturbed = STATUS2 & ECM_DISTURBANCE,
      .ecm_protected =
          (STATUS2 & ECM_PROTECTED) || ECM_ACTIVE || PERSONAL_ECM_ACTIVE,
      .angel_ecm_protected =
          (STATUS2 & ANGEL_ECM_PROTECTED) || ANGEL_ECM_ACTIVE,
      .angel_ecm_disturbed = STATUS2 & ANGEL_ECM_DISTURBED,
      .ecm_countered = ECM_IS_COUNTERED,
      .ecm_destroyed = CRITICAL_STATUS & ECM_DESTROYED,
      .ecm_enabled = STATUS2 & ECM_ENABLED,
      .ecm_active = ECM_ACTIVE,
      .eccm_enabled = STATUS2 & ECCM_ENABLED,
      .angel_ecm_destroyed = CRITICAL_STATUS & ANGEL_ECM_DESTROYED,
      .angel_ecm_enabled = STATUS2 & ANGEL_ECM_ENABLED,
      .angel_ecm_active = ANGEL_ECM_ACTIVE,
      .angel_eccm_enabled = STATUS2 & ANGEL_ECCM_ENABLED,
      .personal_ecm_enabled = STATUS2 & PER_ECM_ENABLED,
      .personal_ecm_active = PERSONAL_ECM_ACTIVE,
      .personal_eccm_enabled = STATUS2 & PER_ECCM_ENABLED,
      .stealth_armor_active = STATUS2 & STH_ARMOR_ON,
      .null_signature_active = STATUS2 & NULLSIGSYS_ON,
      .null_signature_destroyed = CRITICAL_STATUS & NSS_DESTROYED,
      .c3_destroyed = CRITICAL_STATUS & C3_DESTROYED,
      .c3i_destroyed = CRITICAL_STATUS & C3I_DESTROYED,
      .sensors_damaged = CRITICAL_STATUS & SENSORS_DAMAGED,
      .beagle_probe_destroyed = CRITICAL_STATUS & BEAGLE_DESTROYED,
      .bloodhound_probe_destroyed = CRITICAL_STATUS & BLOODHOUND_DESTROYED,
      .light_beagle_probe_destroyed =
          mech->rd.critstatus2 & LIGHT_BAP_DESTROYED,
      .turret_auto_turn = STATUS2 & AUTOTURN_TURRET,
      .arms_flipped = STATUS & FLIPPED_ARMS,
      .targeting_computer_destroyed = CRITICAL_STATUS & TC_DESTROYED,
      .ams_enabled = STATUS & AMS_ENABLED,
      .supercharger_enabled = STATUS & SCHARGE_ENABLED,
      .masc_enabled = STATUS & MASC_ENABLED,
      .player_killer = mech->rd.mech_prefs & MECHPREF_PKILL,
      .friendly_fire_safety = mech->rd.mech_prefs & MECHPREF_NOFRIENDLYFIRE,
      .attack_emissions = STATUS2 & ATTACKEMIT_MECH,
      .unit_target_lock = (STATUS & LOCK_MODES) == LOCK_TARGET,
      .tight_turn_mode = mech->rd.mech_prefs & MECHPREF_TURNMODE,
      .dfa_attacking = STATUS & DFA_ATTACK,
      .turret_jammed = TANK_CRITICAL_STATUS & TURRET_JAMMED,
      .turret_locked = TANK_CRITICAL_STATUS & TURRET_LOCKED,
      .tail_rotor_destroyed = TANK_CRITICAL_STATUS & TAIL_ROTOR_DESTROYED,
      .hip_damaged = CRITICAL_STATUS & HIP_DAMAGED,
      .hip_destroyed = CRITICAL_STATUS & HIP_DESTROYED,
      .gyro_damaged = CRITICAL_STATUS & GYRO_DAMAGED,
      .hardened_gyro_damaged = mech->rd.critstatus2 & HDGYRO_DAMAGED,
      .torso_right = STATUS & TORSO_RIGHT,
      .torso_left = STATUS & TORSO_LEFT,
      .spinning = CRITICAL_STATUS & SPINNING,
      .self_destruct_safe = STATUS & EXPLODE_SAFE,
      .swarm_target = mech->rd.swarming,
      .supercharger_counter = mech->rd.scharge_value,
      .masc_counter = mech->rd.masc_value,
  };
}

bool mech_supercharger_movement_mode_is_enabled(const Mech *mech) {
  return mech->rd.status2 & SCHARGE_ENABLED;
}

typedef struct MechConditionModeToggle {
  int *status;
  int mode;
  int opposite_mode;
} MechConditionModeToggle;

static bool mech_condition_mode_toggle(const MechConditionModeToggle *request) {
  if (*request->status & request->mode) {
    *request->status &= ~request->mode;
    return false;
  }
  *request->status |= request->mode;
  *request->status &= ~request->opposite_mode;
  return true;
}

bool mech_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&(MechConditionModeToggle){
      .status = &mech->rd.status2,
      .mode = eccm ? ECCM_ENABLED : ECM_ENABLED,
      .opposite_mode = eccm ? ECM_ENABLED : ECCM_ENABLED});
}

bool mech_personal_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&(MechConditionModeToggle){
      .status = &mech->rd.status2,
      .mode = eccm ? PER_ECCM_ENABLED : PER_ECM_ENABLED,
      .opposite_mode = eccm ? PER_ECM_ENABLED : PER_ECCM_ENABLED});
}

bool mech_angel_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&(MechConditionModeToggle){
      .status = &mech->rd.status2,
      .mode = eccm ? ANGEL_ECCM_ENABLED : ANGEL_ECM_ENABLED,
      .opposite_mode = eccm ? ANGEL_ECM_ENABLED : ANGEL_ECCM_ENABLED});
}

void mech_torso_twist_set(Mech *mech, MechTorsoTwist twist) {
  mech->rd.status &= ~(TORSO_RIGHT | TORSO_LEFT);
  if (twist == MECH_TORSO_LEFT)
    mech->rd.status |= TORSO_LEFT;
  else if (twist == MECH_TORSO_RIGHT)
    mech->rd.status |= TORSO_RIGHT;
}

void mech_torso_twist_merge(Mech *mech, const Mech *source) {
  mech->rd.status |= source->rd.status & (TORSO_LEFT | TORSO_RIGHT);
}

void mech_arms_center(Mech *mech) { mech->rd.status &= ~FLIPPED_ARMS; }

void mech_arms_flip(Mech *mech) { mech->rd.status |= FLIPPED_ARMS; }

void mech_ams_enabled_set(Mech *mech, bool enabled) {
  if (enabled)
    mech->rd.status |= AMS_ENABLED;
  else
    mech->rd.status &= ~AMS_ENABLED;
}

void mech_evading_set(Mech *mech, bool evading) {
  if (evading)
    mech->rd.status2 |= EVADING;
  else
    mech->rd.status2 &= ~EVADING;
}

void mech_sprinting_set(Mech *mech, bool sprinting) {
  if (sprinting)
    mech->rd.status2 |= SPRINTING;
  else
    mech->rd.status2 &= ~SPRINTING;
}

void mech_dfa_attacking_set(Mech *mech, bool attacking) {
  if (attacking)
    mech->rd.status |= DFA_ATTACK;
  else
    mech->rd.status &= ~DFA_ATTACK;
}

void mech_turret_auto_turn_set(Mech *mech, bool enabled) {
  if (enabled)
    mech->rd.status2 |= AUTOTURN_TURRET;
  else
    mech->rd.status2 &= ~AUTOTURN_TURRET;
}

void mech_tight_turn_mode_set(Mech *mech, bool enabled) {
  if (enabled)
    mech->rd.mech_prefs |= MECHPREF_TURNMODE;
  else
    mech->rd.mech_prefs &= ~MECHPREF_TURNMODE;
}

void mech_player_killer_set(Mech *mech, bool enabled) {
  if (enabled)
    mech->rd.mech_prefs |= MECHPREF_PKILL;
  else
    mech->rd.mech_prefs &= ~MECHPREF_PKILL;
}

void mech_partial_cover_set(Mech *mech, bool covered) {
  if (covered)
    mech->rd.status |= PARTIAL_COVER;
  else
    mech->rd.status &= ~PARTIAL_COVER;
}

DbRef mech_swarm_target(const Mech *mech) { return mech->rd.swarming; }

void mech_swarm_target_set(Mech *mech, DbRef target) {
  mech->rd.swarming = target;
}

DbRef mech_swarmed_by(const Mech *mech) { return mech->rd.swarmedby; }

void mech_swarmed_by_set(Mech *mech, DbRef swarmer) {
  mech->rd.swarmedby = swarmer;
}

bool mech_is_mounting(const Mech *mech) {
  return mech->rd.status2 & UNIT_MOUNTING;
}

void mech_mounting_set(Mech *mech, bool mounting) {
  if (mounting)
    mech->rd.status2 |= UNIT_MOUNTING;
  else
    mech->rd.status2 &= ~UNIT_MOUNTING;
}

bool mech_is_mounted(const Mech *mech) {
  return mech->rd.status2 & UNIT_MOUNTED;
}

void mech_mounted_set(Mech *mech, bool mounted) {
  if (mounted)
    mech->rd.status2 |= UNIT_MOUNTED;
  else
    mech->rd.status2 &= ~UNIT_MOUNTED;
}

void mech_fallen_set(Mech *mech, bool fallen) {
  if (fallen)
    mech->rd.status |= FALLEN;
  else
    mech->rd.status &= ~FALLEN;
}

void mech_hull_down_set(Mech *mech, bool hull_down) {
  if (hull_down)
    mech->rd.status |= HULLDOWN;
  else
    mech->rd.status &= ~HULLDOWN;
}

void mech_dug_in_set(Mech *mech, bool dug_in) {
  if (dug_in)
    mech->rd.tankcritstatus |= DUG_IN;
  else
    mech->rd.tankcritstatus &= ~DUG_IN;
}

void mech_digging_set(Mech *mech, bool digging) {
  if (digging)
    mech->rd.tankcritstatus |= DIGGING_IN;
  else
    mech->rd.tankcritstatus &= ~DIGGING_IN;
}

void mech_hidden_set(Mech *mech, bool hidden) {
  if (hidden)
    mech->rd.critstatus |= HIDDEN;
  else
    mech->rd.critstatus &= ~HIDDEN;
}

void mech_spinning_set(Mech *mech, bool spinning) {
  if (spinning)
    mech->rd.critstatus |= SPINNING;
  else
    mech->rd.critstatus &= ~SPINNING;
}

void mech_masc_enabled_set(Mech *mech, bool enabled) {
  if (enabled)
    mech->rd.status |= MASC_ENABLED;
  else
    mech->rd.status &= ~MASC_ENABLED;
}

void mech_supercharger_enabled_set(Mech *mech, bool enabled) {
  if (enabled)
    mech->rd.status |= SCHARGE_ENABLED;
  else
    mech->rd.status &= ~SCHARGE_ENABLED;
}

int mech_masc_counter_advance(Mech *mech) { return mech->rd.masc_value++; }

bool mech_masc_counter_regenerate(Mech *mech) {
  if (mech->rd.masc_value <= 0)
    return false;
  mech->rd.masc_value--;
  return true;
}

int mech_supercharger_counter_advance(Mech *mech) {
  return mech->rd.scharge_value++;
}

bool mech_supercharger_counter_regenerate(Mech *mech) {
  if (mech->rd.scharge_value <= 0)
    return false;
  mech->rd.scharge_value--;
  return true;
}

void mech_hip_damage_set(Mech *mech, bool damaged, bool destroyed) {
  if (damaged)
    mech->rd.critstatus |= HIP_DAMAGED;
  else
    mech->rd.critstatus &= ~HIP_DAMAGED;
  if (destroyed)
    mech->rd.critstatus |= HIP_DESTROYED;
  else
    mech->rd.critstatus &= ~HIP_DESTROYED;
}

void mech_turret_jammed_set(Mech *mech, bool jammed) {
  if (jammed)
    mech->rd.tankcritstatus |= TURRET_JAMMED;
  else
    mech->rd.tankcritstatus &= ~TURRET_JAMMED;
}

void mech_turret_locked_set(Mech *mech, bool locked) {
  if (locked)
    mech->rd.tankcritstatus |= TURRET_LOCKED;
  else
    mech->rd.tankcritstatus &= ~TURRET_LOCKED;
}

void mech_crew_stunned_set(Mech *mech, bool stunned) {
  if (stunned)
    mech->rd.tankcritstatus |= CREW_STUNNED;
  else
    mech->rd.tankcritstatus &= ~CREW_STUNNED;
}

void mech_tail_rotor_destroyed_set(Mech *mech, bool destroyed) {
  if (destroyed)
    mech->rd.tankcritstatus |= TAIL_ROTOR_DESTROYED;
  else
    mech->rd.tankcritstatus &= ~TAIL_ROTOR_DESTROYED;
}

static void mech_critical_status_set(Mech *mech, int flag, bool enabled) {
  if (enabled)
    mech->rd.critstatus |= flag;
  else
    mech->rd.critstatus &= ~flag;
}

void mech_life_support_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, LIFE_SUPPORT_DESTROYED, destroyed);
}

void mech_sensors_damaged_set(Mech *mech, bool damaged) {
  mech_critical_status_set(mech, SENSORS_DAMAGED, damaged);
}

void mech_targeting_computer_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, TC_DESTROYED, destroyed);
}

void mech_gyro_damage_set(Mech *mech, bool damaged, bool destroyed) {
  mech_critical_status_set(mech, GYRO_DAMAGED, damaged);
  mech_critical_status_set(mech, GYRO_DESTROYED, destroyed);
}

void mech_hardened_gyro_damaged_set(Mech *mech, bool damaged) {
  if (damaged)
    mech->rd.critstatus2 |= HDGYRO_DAMAGED;
  else
    mech->rd.critstatus2 &= ~HDGYRO_DAMAGED;
}

void mech_c3_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, C3_DESTROYED, destroyed);
}

void mech_c3i_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, C3I_DESTROYED, destroyed);
}

void mech_tag_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, TAG_DESTROYED, destroyed);
}

void mech_ecm_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, ECM_DESTROYED, destroyed);
}

void mech_angel_ecm_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, ANGEL_ECM_DESTROYED, destroyed);
}

void mech_ecm_modes_disable(Mech *mech) {
  mech->rd.status2 &= ~(ECM_ENABLED | ECCM_ENABLED);
}

void mech_angel_ecm_modes_disable(Mech *mech) {
  mech->rd.status2 &= ~(ANGEL_ECM_ENABLED | ANGEL_ECCM_ENABLED);
}

void mech_beagle_probe_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, BEAGLE_DESTROYED, destroyed);
}

void mech_bloodhound_probe_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, BLOODHOUND_DESTROYED, destroyed);
}

void mech_light_beagle_probe_destroyed_set(Mech *mech, bool destroyed) {
  if (destroyed)
    mech->rd.critstatus2 |= LIGHT_BAP_DESTROYED;
  else
    mech->rd.critstatus2 &= ~LIGHT_BAP_DESTROYED;
}

void mech_null_signature_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, NSS_DESTROYED, destroyed);
}

bool mech_section_is_underwater(const Mech *mech, int section) {
  if (mech->pd.z >= 0)
    return false;
  if (mech->pd.z < -1 || (mech->rd.status & FALLEN))
    return true;
  return section == LLEG || section == RLEG ||
         (mech->ud.type == CLASS_MECH && mech->ud.move == MOVE_QUAD &&
          (section == LARM || section == RARM));
}

void mech_stunned_set(Mech *mech, bool stunned) {
  if (stunned)
    mech->rd.critstatus |= MECH_STUNNED;
  else
    mech->rd.critstatus &= ~MECH_STUNNED;
}

void mech_performing_action_set(Mech *mech, bool performing) {
  if (performing)
    mech->rd.status |= PERFORMING_ACTION;
  else
    mech->rd.status &= ~PERFORMING_ACTION;
}

void mech_searchlight_set(Mech *mech, bool enabled) {
  if (enabled) {
    mech->rd.status2 |= SLITE_ON;
    mech->rd.critstatus |= SLITE_LIT;
  } else {
    mech->rd.status2 &= ~SLITE_ON;
    mech->rd.critstatus &= ~SLITE_LIT;
  }
}

void mech_searchlight_destroy(Mech *mech) {
  mech->rd.critstatus |= SLITE_DEST;
  mech->rd.status2 &= ~SLITE_ON;
}

void mech_stealth_armor_active_set(Mech *mech, bool active) {
  if (active)
    mech->rd.status2 |= STH_ARMOR_ON;
  else
    mech->rd.status2 &= ~STH_ARMOR_ON;
}

void mech_null_signature_active_set(Mech *mech, bool active) {
  if (active)
    mech->rd.status2 |= NULLSIGSYS_ON;
  else
    mech->rd.status2 &= ~NULLSIGSYS_ON;
}
