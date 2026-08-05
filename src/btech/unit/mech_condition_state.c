#include "mech_condition_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

MechConditionSummary mech_condition_summary(const Mech *mech) {
  const int status = mech->rd.status;
  const int status2 = mech->rd.status2;
  const int critical_status = mech->rd.critstatus;
  const int tank_critical_status = mech->rd.tankcritstatus;
  const bool ecm_countered = status2 & ECM_COUNTERED;
  const bool ecm_active = (status2 & ECM_ENABLED) && !ecm_countered;
  const bool personal_ecm_active =
      (status2 & PER_ECM_ENABLED) && !ecm_countered;
  const bool angel_ecm_active = (status2 & ANGEL_ECM_ENABLED) && !ecm_countered;

  return (MechConditionSummary){
      .combat_safe = status & COMBAT_SAFE,
      .partial_cover = status & PARTIAL_COVER,
      .fallen = status & FALLEN,
      .fortified = status2 & FORTIFIED,
      .weapons_hold = status2 & WEAPONS_HOLD,
      .hull_down = status & HULLDOWN,
      .dug_in = tank_critical_status & DUG_IN,
      .digging = tank_critical_status & DIGGING_IN,
      .staggering = mech->rd.staggerDamage / 20 > 0,
      .searchlight_destroyed = critical_status & SLITE_DEST,
      .searchlight_on = status2 & SLITE_ON,
      .illuminated = critical_status & SLITE_LIT,
      .hidden = critical_status & HIDDEN,
      .dodging = status2 & DODGING,
      .evading = status2 & EVADING,
      .sprinting = status2 & SPRINTING,
      .stunned = (tank_critical_status & CREW_STUNNED) ||
                 (critical_status & MECH_STUNNED),
      .performing_action = status & PERFORMING_ACTION,
      .auto_fall = mech->rd.mech_prefs & MECHPREF_AUTOFALL,
      .to_hit_debug = mech->rd.mech_prefs & MECHPREF_BTHDEBUG,
      .ecm_protected =
          (status2 & ECM_PROTECTED) || ecm_active || personal_ecm_active,
      .angel_ecm_protected =
          (status2 & ANGEL_ECM_PROTECTED) || angel_ecm_active,
      .angel_ecm_disturbed = status2 & ANGEL_ECM_DISTURBED,
      .ecm_countered = ecm_countered,
      .ecm_destroyed = critical_status & ECM_DESTROYED,
      .ecm_enabled = status2 & ECM_ENABLED,
      .ecm_active = ecm_active,
      .eccm_enabled = status2 & ECCM_ENABLED,
      .angel_ecm_destroyed = critical_status & ANGEL_ECM_DESTROYED,
      .angel_ecm_enabled = status2 & ANGEL_ECM_ENABLED,
      .angel_ecm_active = angel_ecm_active,
      .angel_eccm_enabled = status2 & ANGEL_ECCM_ENABLED,
      .personal_ecm_enabled = status2 & PER_ECM_ENABLED,
      .personal_ecm_active = personal_ecm_active,
      .personal_eccm_enabled = status2 & PER_ECCM_ENABLED,
      .stealth_armor_active = status2 & STH_ARMOR_ON,
      .null_signature_active = status2 & NULLSIGSYS_ON,
      .null_signature_destroyed = critical_status & NSS_DESTROYED,
      .c3_destroyed = critical_status & C3_DESTROYED,
      .c3i_destroyed = critical_status & C3I_DESTROYED,
      .sensors_damaged = critical_status & SENSORS_DAMAGED,
      .beagle_probe_destroyed = critical_status & BEAGLE_DESTROYED,
      .bloodhound_probe_destroyed = critical_status & BLOODHOUND_DESTROYED,
      .light_beagle_probe_destroyed =
          mech->rd.critstatus2 & LIGHT_BAP_DESTROYED,
      .turret_auto_turn = status2 & AUTOTURN_TURRET,
      .arms_flipped = status & FLIPPED_ARMS,
      .targeting_computer_destroyed = critical_status & TC_DESTROYED,
      .ams_enabled = status & AMS_ENABLED,
      .supercharger_enabled = status & SCHARGE_ENABLED,
      .masc_enabled = status & MASC_ENABLED,
      .player_killer = mech->rd.mech_prefs & MECHPREF_PKILL,
      .tight_turn_mode = mech->rd.mech_prefs & MECHPREF_TURNMODE,
      .dfa_attacking = status & DFA_ATTACK,
      .turret_jammed = tank_critical_status & TURRET_JAMMED,
      .turret_locked = tank_critical_status & TURRET_LOCKED,
      .tail_rotor_destroyed = tank_critical_status & TAIL_ROTOR_DESTROYED,
      .hip_damaged = critical_status & HIP_DAMAGED,
      .hip_destroyed = critical_status & HIP_DESTROYED,
      .gyro_damaged = critical_status & GYRO_DAMAGED,
      .hardened_gyro_damaged = mech->rd.critstatus2 & HDGYRO_DAMAGED,
      .torso_right = status & TORSO_RIGHT,
      .torso_left = status & TORSO_LEFT,
      .spinning = critical_status & SPINNING,
      .self_destruct_safe = status & EXPLODE_SAFE,
      .swarm_target = mech->rd.swarming,
      .supercharger_counter = mech->rd.scharge_value,
      .masc_counter = mech->rd.masc_value,
  };
}

bool mech_supercharger_movement_mode_is_enabled(const Mech *mech) {
  return mech->rd.status2 & SCHARGE_ENABLED;
}

static bool mech_condition_mode_toggle(int *status, int mode, int opposite) {
  if (*status & mode) {
    *status &= ~mode;
    return false;
  }
  *status |= mode;
  *status &= ~opposite;
  return true;
}

bool mech_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&mech->rd.status2,
                                    eccm ? ECCM_ENABLED : ECM_ENABLED,
                                    eccm ? ECM_ENABLED : ECCM_ENABLED);
}

bool mech_personal_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&mech->rd.status2,
                                    eccm ? PER_ECCM_ENABLED : PER_ECM_ENABLED,
                                    eccm ? PER_ECM_ENABLED : PER_ECCM_ENABLED);
}

bool mech_angel_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(
      &mech->rd.status2, eccm ? ANGEL_ECCM_ENABLED : ANGEL_ECM_ENABLED,
      eccm ? ANGEL_ECM_ENABLED : ANGEL_ECCM_ENABLED);
}

void mech_torso_twist_set(Mech *mech, MechTorsoTwist twist) {
  mech->rd.status &= ~(TORSO_RIGHT | TORSO_LEFT);
  if (twist == MECH_TORSO_LEFT)
    mech->rd.status |= TORSO_LEFT;
  else if (twist == MECH_TORSO_RIGHT)
    mech->rd.status |= TORSO_RIGHT;
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

void mech_swarm_target_set(Mech *mech, DbRef target) {
  mech->rd.swarming = target;
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

void mech_searchlight_set(Mech *mech, bool enabled) {
  if (enabled) {
    mech->rd.status2 |= SLITE_ON;
    mech->rd.critstatus |= SLITE_LIT;
  } else {
    mech->rd.status2 &= ~SLITE_ON;
    mech->rd.critstatus &= ~SLITE_LIT;
  }
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
