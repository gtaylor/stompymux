#include "equipment_types.h"
#include "mech_condition_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"
#include "mux/server/platform.h"
#include "section_types.h"

MechConditionSummary mech_condition_summary(const Mech *mech) {
  const MechStatus STATUS = mech->rd.status;
  const MechStatus2 STATUS2 = mech->rd.status2;
  const MechCritStatus CRITICAL_STATUS = mech->rd.critstatus;
  const MechTankCritStatus TANK_CRITICAL_STATUS = mech->rd.tankcritstatus;
  const bool ECM_IS_COUNTERED =
      mech_status2_has(STATUS2, MECH_STATUS2_ECM_COUNTERED);
  const bool ECM_ACTIVE =
      (mech_status2_has(STATUS2, MECH_STATUS2_ECM_ENABLED) &&
       !ECM_IS_COUNTERED) != 0;
  const bool PERSONAL_ECM_ACTIVE =
      (mech_status2_has(STATUS2, MECH_STATUS2_PER_ECM_ENABLED) &&
       !ECM_IS_COUNTERED) != 0;
  const bool ANGEL_ECM_ACTIVE =
      (mech_status2_has(STATUS2, MECH_STATUS2_ANGEL_ECM_ENABLED) &&
       !ECM_IS_COUNTERED) != 0;

  return (MechConditionSummary){
      .combat_safe = mech_status_has(STATUS, MECH_STATUS_COMBAT_SAFE),
      .partial_cover = mech_status_has(STATUS, MECH_STATUS_PARTIAL_COVER),
      .fallen = mech_status_has(STATUS, MECH_STATUS_FALLEN),
      .fortified = mech_status2_has(STATUS2, MECH_STATUS2_FORTIFIED),
      .weapons_hold = mech_status2_has(STATUS2, MECH_STATUS2_WEAPONS_HOLD),
      .hull_down = mech_status_has(STATUS, MECH_STATUS_HULLDOWN),
      .dug_in = mech_tank_crit_status_has(TANK_CRITICAL_STATUS,
                                          MECH_TANK_CRIT_STATUS_DUG_IN),
      .digging = mech_tank_crit_status_has(TANK_CRITICAL_STATUS,
                                           MECH_TANK_CRIT_STATUS_DIGGING_IN),
      .staggering = mech->rd.stagger_damage / 20 > 0,
      .searchlight_destroyed =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_SLITE_DEST),
      .searchlight_on = mech_status2_has(STATUS2, MECH_STATUS2_SLITE_ON),
      .illuminated =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_SLITE_LIT),
      .hidden = mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_HIDDEN),
      .dodging = mech_status2_has(STATUS2, MECH_STATUS2_DODGING),
      .evading = mech_status2_has(STATUS2, MECH_STATUS2_EVADING),
      .sprinting = mech_status2_has(STATUS2, MECH_STATUS2_SPRINTING),
      .stunned =
          (mech_tank_crit_status_has(TANK_CRITICAL_STATUS,
                                     MECH_TANK_CRIT_STATUS_CREW_STUNNED) ||
           mech_crit_status_has(CRITICAL_STATUS,
                                MECH_CRIT_STATUS_MECH_STUNNED)) != 0,
      .performing_action =
          mech_status_has(STATUS, MECH_STATUS_PERFORMING_ACTION),
      .auto_fall = (mech->rd.mech_prefs & MECHPREF_AUTOFALL) != 0,
      .to_hit_debug = (mech->rd.mech_prefs & MECHPREF_BTHDEBUG) != 0,
      .ecm_disturbed = mech_status2_has(STATUS2, MECH_STATUS2_ECM_DISTURBANCE),
      .ecm_protected = (mech_status2_has(STATUS2, MECH_STATUS2_ECM_PROTECTED) ||
                        ECM_ACTIVE || PERSONAL_ECM_ACTIVE) != 0,
      .angel_ecm_protected =
          (mech_status2_has(STATUS2, MECH_STATUS2_ANGEL_ECM_PROTECTED) ||
           ANGEL_ECM_ACTIVE) != 0,
      .angel_ecm_disturbed =
          mech_status2_has(STATUS2, MECH_STATUS2_ANGEL_ECM_DISTURBED),
      .ecm_countered = ECM_IS_COUNTERED,
      .ecm_destroyed =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_ECM_DESTROYED),
      .ecm_enabled = mech_status2_has(STATUS2, MECH_STATUS2_ECM_ENABLED),
      .ecm_active = ECM_ACTIVE,
      .eccm_enabled = mech_status2_has(STATUS2, MECH_STATUS2_ECCM_ENABLED),
      .angel_ecm_destroyed = mech_crit_status_has(
          CRITICAL_STATUS, MECH_CRIT_STATUS_ANGEL_ECM_DESTROYED),
      .angel_ecm_enabled =
          mech_status2_has(STATUS2, MECH_STATUS2_ANGEL_ECM_ENABLED),
      .angel_ecm_active = ANGEL_ECM_ACTIVE,
      .angel_eccm_enabled =
          mech_status2_has(STATUS2, MECH_STATUS2_ANGEL_ECCM_ENABLED),
      .personal_ecm_enabled =
          mech_status2_has(STATUS2, MECH_STATUS2_PER_ECM_ENABLED),
      .personal_ecm_active = PERSONAL_ECM_ACTIVE,
      .personal_eccm_enabled =
          mech_status2_has(STATUS2, MECH_STATUS2_PER_ECCM_ENABLED),
      .stealth_armor_active =
          mech_status2_has(STATUS2, MECH_STATUS2_STH_ARMOR_ON),
      .null_signature_active =
          mech_status2_has(STATUS2, MECH_STATUS2_NULLSIGSYS_ON),
      .null_signature_destroyed =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_NSS_DESTROYED),
      .c3_destroyed =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_C3_DESTROYED),
      .c3i_destroyed =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_C3I_DESTROYED),
      .sensors_damaged = mech_crit_status_has(CRITICAL_STATUS,
                                              MECH_CRIT_STATUS_SENSORS_DAMAGED),
      .beagle_probe_destroyed = mech_crit_status_has(
          CRITICAL_STATUS, MECH_CRIT_STATUS_BEAGLE_DESTROYED),
      .bloodhound_probe_destroyed = mech_crit_status_has(
          CRITICAL_STATUS, MECH_CRIT_STATUS_BLOODHOUND_DESTROYED),
      .light_beagle_probe_destroyed = mech_crit_status2_has(
          mech->rd.critstatus2, MECH_CRIT_STATUS2_LIGHT_BAP_DESTROYED),
      .turret_auto_turn =
          mech_status2_has(STATUS2, MECH_STATUS2_AUTOTURN_TURRET),
      .arms_flipped = mech_status_has(STATUS, MECH_STATUS_FLIPPED_ARMS),
      .targeting_computer_destroyed =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_TC_DESTROYED),
      .ams_enabled = mech_status_has(STATUS, MECH_STATUS_AMS_ENABLED),
      .supercharger_enabled =
          mech_status_has(STATUS, MECH_STATUS_SCHARGE_ENABLED),
      .masc_enabled = mech_status_has(STATUS, MECH_STATUS_MASC_ENABLED),
      .player_killer = (mech->rd.mech_prefs & MECHPREF_PKILL) != 0,
      .friendly_fire_safety =
          (mech->rd.mech_prefs & MECHPREF_NOFRIENDLYFIRE) != 0,
      .attack_emissions =
          mech_status2_has(STATUS2, MECH_STATUS2_ATTACKEMIT_MECH),
      .unit_target_lock = mech_status_mask(STATUS, MECH_STATUS_LOCK_MODES) ==
                          MECH_STATUS_LOCK_TARGET,
      .tight_turn_mode = (mech->rd.mech_prefs & MECHPREF_TURNMODE) != 0,
      .dfa_attacking = mech_status_has(STATUS, MECH_STATUS_DFA_ATTACK),
      .turret_jammed = mech_tank_crit_status_has(
          TANK_CRITICAL_STATUS, MECH_TANK_CRIT_STATUS_TURRET_JAMMED),
      .turret_locked = mech_tank_crit_status_has(
          TANK_CRITICAL_STATUS, MECH_TANK_CRIT_STATUS_TURRET_LOCKED),
      .tail_rotor_destroyed = mech_tank_crit_status_has(
          TANK_CRITICAL_STATUS, MECH_TANK_CRIT_STATUS_TAIL_ROTOR_DESTROYED),
      .hip_damaged =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_HIP_DAMAGED),
      .hip_destroyed =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_HIP_DESTROYED),
      .gyro_damaged =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_GYRO_DAMAGED),
      .hardened_gyro_damaged = mech_crit_status2_has(
          mech->rd.critstatus2, MECH_CRIT_STATUS2_HDGYRO_DAMAGED),
      .torso_right = mech_status_has(STATUS, MECH_STATUS_TORSO_RIGHT),
      .torso_left = mech_status_has(STATUS, MECH_STATUS_TORSO_LEFT),
      .spinning =
          mech_crit_status_has(CRITICAL_STATUS, MECH_CRIT_STATUS_SPINNING),
      .self_destruct_safe = mech_status_has(STATUS, MECH_STATUS_EXPLODE_SAFE),
      .swarm_target = mech->rd.swarming,
      .supercharger_counter = mech->rd.scharge_value,
      .masc_counter = mech->rd.masc_value,
  };
}

bool mech_supercharger_movement_mode_is_enabled(const Mech *mech) {
  return mech_status2_has(mech->rd.status2, MECH_STATUS2_SCHARGE_ENABLED);
}

typedef struct MechConditionModeToggle {
  MechStatus2 *status;
  MechStatus2 mode;
  MechStatus2 opposite_mode;
} MechConditionModeToggle;

static bool mech_condition_mode_toggle(const MechConditionModeToggle *request) {
  if (mech_status2_has(*request->status, request->mode)) {
    mech_status2_clear(request->status, request->mode);
    return false;
  }
  mech_status2_set(request->status, request->mode);
  mech_status2_clear(request->status, request->opposite_mode);
  return true;
}

bool mech_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&(MechConditionModeToggle){
      .status = &mech->rd.status2,
      .mode = eccm ? MECH_STATUS2_ECCM_ENABLED : MECH_STATUS2_ECM_ENABLED,
      .opposite_mode =
          eccm ? MECH_STATUS2_ECM_ENABLED : MECH_STATUS2_ECCM_ENABLED});
}

bool mech_personal_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&(MechConditionModeToggle){
      .status = &mech->rd.status2,
      .mode =
          eccm ? MECH_STATUS2_PER_ECCM_ENABLED : MECH_STATUS2_PER_ECM_ENABLED,
      .opposite_mode =
          eccm ? MECH_STATUS2_PER_ECM_ENABLED : MECH_STATUS2_PER_ECCM_ENABLED});
}

bool mech_angel_ecm_mode_toggle(Mech *mech, bool eccm) {
  return mech_condition_mode_toggle(&(MechConditionModeToggle){
      .status = &mech->rd.status2,
      .mode = eccm ? MECH_STATUS2_ANGEL_ECCM_ENABLED
                   : MECH_STATUS2_ANGEL_ECM_ENABLED,
      .opposite_mode = eccm ? MECH_STATUS2_ANGEL_ECM_ENABLED
                            : MECH_STATUS2_ANGEL_ECCM_ENABLED});
}

void mech_torso_twist_set(Mech *mech, MechTorsoTwist twist) {
  mech_status_clear(&mech->rd.status, (MechStatus)(MECH_STATUS_TORSO_RIGHT |
                                                   MECH_STATUS_TORSO_LEFT));
  if (twist == MECH_TORSO_LEFT)
    mech_status_set(&mech->rd.status, MECH_STATUS_TORSO_LEFT);
  else if (twist == MECH_TORSO_RIGHT)
    mech_status_set(&mech->rd.status, MECH_STATUS_TORSO_RIGHT);
}

void mech_torso_twist_merge(Mech *mech, const Mech *source) {
  mech_status_set(&mech->rd.status,
                  mech_status_mask(source->rd.status,
                                   (MechStatus)(MECH_STATUS_TORSO_LEFT |
                                                MECH_STATUS_TORSO_RIGHT)));
}

void mech_arms_center(Mech *mech) {
  mech_status_clear(&mech->rd.status, MECH_STATUS_FLIPPED_ARMS);
}

void mech_arms_flip(Mech *mech) {
  mech_status_set(&mech->rd.status, MECH_STATUS_FLIPPED_ARMS);
}

void mech_ams_enabled_set(Mech *mech, bool enabled) {
  if (enabled)
    mech_status_set(&mech->rd.status, MECH_STATUS_AMS_ENABLED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_AMS_ENABLED);
}

void mech_evading_set(Mech *mech, bool evading) {
  if (evading)
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_EVADING);
  else
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_EVADING);
}

void mech_sprinting_set(Mech *mech, bool sprinting) {
  if (sprinting)
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_SPRINTING);
  else
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_SPRINTING);
}

void mech_dfa_attacking_set(Mech *mech, bool attacking) {
  if (attacking)
    mech_status_set(&mech->rd.status, MECH_STATUS_DFA_ATTACK);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_DFA_ATTACK);
}

void mech_turret_auto_turn_set(Mech *mech, bool enabled) {
  if (enabled)
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_AUTOTURN_TURRET);
  else
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_AUTOTURN_TURRET);
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
    mech_status_set(&mech->rd.status, MECH_STATUS_PARTIAL_COVER);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_PARTIAL_COVER);
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
  return mech_status2_has(mech->rd.status2, MECH_STATUS2_UNIT_MOUNTING);
}

void mech_mounting_set(Mech *mech, bool mounting) {
  if (mounting)
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_UNIT_MOUNTING);
  else
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_UNIT_MOUNTING);
}

bool mech_is_mounted(const Mech *mech) {
  return mech_status2_has(mech->rd.status2, MECH_STATUS2_UNIT_MOUNTED);
}

void mech_mounted_set(Mech *mech, bool mounted) {
  if (mounted)
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_UNIT_MOUNTED);
  else
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_UNIT_MOUNTED);
}

void mech_fallen_set(Mech *mech, bool fallen) {
  if (fallen)
    mech_status_set(&mech->rd.status, MECH_STATUS_FALLEN);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_FALLEN);
}

void mech_hull_down_set(Mech *mech, bool hull_down) {
  if (hull_down)
    mech_status_set(&mech->rd.status, MECH_STATUS_HULLDOWN);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_HULLDOWN);
}

void mech_dug_in_set(Mech *mech, bool dug_in) {
  if (dug_in)
    mech_tank_crit_status_set(&mech->rd.tankcritstatus,
                              MECH_TANK_CRIT_STATUS_DUG_IN);
  else
    mech_tank_crit_status_clear(&mech->rd.tankcritstatus,
                                MECH_TANK_CRIT_STATUS_DUG_IN);
}

void mech_digging_set(Mech *mech, bool digging) {
  if (digging)
    mech_tank_crit_status_set(&mech->rd.tankcritstatus,
                              MECH_TANK_CRIT_STATUS_DIGGING_IN);
  else
    mech_tank_crit_status_clear(&mech->rd.tankcritstatus,
                                MECH_TANK_CRIT_STATUS_DIGGING_IN);
}

void mech_hidden_set(Mech *mech, bool hidden) {
  if (hidden)
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_HIDDEN);
  else
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_HIDDEN);
}

void mech_spinning_set(Mech *mech, bool spinning) {
  if (spinning)
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_SPINNING);
  else
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_SPINNING);
}

void mech_masc_enabled_set(Mech *mech, bool enabled) {
  if (enabled)
    mech_status_set(&mech->rd.status, MECH_STATUS_MASC_ENABLED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_MASC_ENABLED);
}

void mech_supercharger_enabled_set(Mech *mech, bool enabled) {
  if (enabled)
    mech_status_set(&mech->rd.status, MECH_STATUS_SCHARGE_ENABLED);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_SCHARGE_ENABLED);
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
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_HIP_DAMAGED);
  else
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_HIP_DAMAGED);
  if (destroyed)
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_HIP_DESTROYED);
  else
    mech_crit_status_clear(&mech->rd.critstatus,
                           MECH_CRIT_STATUS_HIP_DESTROYED);
}

void mech_turret_jammed_set(Mech *mech, bool jammed) {
  if (jammed)
    mech_tank_crit_status_set(&mech->rd.tankcritstatus,
                              MECH_TANK_CRIT_STATUS_TURRET_JAMMED);
  else
    mech_tank_crit_status_clear(&mech->rd.tankcritstatus,
                                MECH_TANK_CRIT_STATUS_TURRET_JAMMED);
}

void mech_turret_locked_set(Mech *mech, bool locked) {
  if (locked)
    mech_tank_crit_status_set(&mech->rd.tankcritstatus,
                              MECH_TANK_CRIT_STATUS_TURRET_LOCKED);
  else
    mech_tank_crit_status_clear(&mech->rd.tankcritstatus,
                                MECH_TANK_CRIT_STATUS_TURRET_LOCKED);
}

void mech_crew_stunned_set(Mech *mech, bool stunned) {
  if (stunned)
    mech_tank_crit_status_set(&mech->rd.tankcritstatus,
                              MECH_TANK_CRIT_STATUS_CREW_STUNNED);
  else
    mech_tank_crit_status_clear(&mech->rd.tankcritstatus,
                                MECH_TANK_CRIT_STATUS_CREW_STUNNED);
}

void mech_tail_rotor_destroyed_set(Mech *mech, bool destroyed) {
  if (destroyed)
    mech_tank_crit_status_set(&mech->rd.tankcritstatus,
                              MECH_TANK_CRIT_STATUS_TAIL_ROTOR_DESTROYED);
  else
    mech_tank_crit_status_clear(&mech->rd.tankcritstatus,
                                MECH_TANK_CRIT_STATUS_TAIL_ROTOR_DESTROYED);
}

static void mech_critical_status_set(Mech *mech, MechCritStatus flag,
                                     bool enabled) {
  if (enabled)
    mech_crit_status_set(&mech->rd.critstatus, flag);
  else
    mech_crit_status_clear(&mech->rd.critstatus, flag);
}

void mech_life_support_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_LIFE_SUPPORT_DESTROYED,
                           destroyed);
}

void mech_sensors_damaged_set(Mech *mech, bool damaged) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_SENSORS_DAMAGED, damaged);
}

void mech_targeting_computer_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_TC_DESTROYED, destroyed);
}

void mech_gyro_damage_set(Mech *mech, bool damaged, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_GYRO_DAMAGED, damaged);
  mech_critical_status_set(mech, MECH_CRIT_STATUS_GYRO_DESTROYED, destroyed);
}

void mech_hardened_gyro_damaged_set(Mech *mech, bool damaged) {
  if (damaged)
    mech_crit_status2_set(&mech->rd.critstatus2,
                          MECH_CRIT_STATUS2_HDGYRO_DAMAGED);
  else
    mech_crit_status2_clear(&mech->rd.critstatus2,
                            MECH_CRIT_STATUS2_HDGYRO_DAMAGED);
}

void mech_c3_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_C3_DESTROYED, destroyed);
}

void mech_c3i_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_C3I_DESTROYED, destroyed);
}

void mech_tag_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_TAG_DESTROYED, destroyed);
}

void mech_ecm_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_ECM_DESTROYED, destroyed);
}

void mech_angel_ecm_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_ANGEL_ECM_DESTROYED,
                           destroyed);
}

void mech_ecm_modes_disable(Mech *mech) {
  mech_status2_clear(
      &mech->rd.status2,
      (MechStatus2)(MECH_STATUS2_ECM_ENABLED | MECH_STATUS2_ECCM_ENABLED));
}

void mech_angel_ecm_modes_disable(Mech *mech) {
  mech_status2_clear(&mech->rd.status2,
                     (MechStatus2)(MECH_STATUS2_ANGEL_ECM_ENABLED |
                                   MECH_STATUS2_ANGEL_ECCM_ENABLED));
}

void mech_beagle_probe_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_BEAGLE_DESTROYED, destroyed);
}

void mech_bloodhound_probe_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_BLOODHOUND_DESTROYED,
                           destroyed);
}

void mech_light_beagle_probe_destroyed_set(Mech *mech, bool destroyed) {
  if (destroyed)
    mech_crit_status2_set(&mech->rd.critstatus2,
                          MECH_CRIT_STATUS2_LIGHT_BAP_DESTROYED);
  else
    mech_crit_status2_clear(&mech->rd.critstatus2,
                            MECH_CRIT_STATUS2_LIGHT_BAP_DESTROYED);
}

void mech_null_signature_destroyed_set(Mech *mech, bool destroyed) {
  mech_critical_status_set(mech, MECH_CRIT_STATUS_NSS_DESTROYED, destroyed);
}

bool mech_section_is_underwater(const Mech *mech, int section) {
  if (mech->pd.z >= 0)
    return false;
  if (mech->pd.z < -1 || mech_status_has(mech->rd.status, MECH_STATUS_FALLEN))
    return true;
  return (section == LLEG || section == RLEG ||
          (mech->ud.type == CLASS_MECH && mech->ud.move == MOVE_QUAD &&
           (section == LARM || section == RARM))) != 0;
}

void mech_stunned_set(Mech *mech, bool stunned) {
  if (stunned)
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_MECH_STUNNED);
  else
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_MECH_STUNNED);
}

void mech_performing_action_set(Mech *mech, bool performing) {
  if (performing)
    mech_status_set(&mech->rd.status, MECH_STATUS_PERFORMING_ACTION);
  else
    mech_status_clear(&mech->rd.status, MECH_STATUS_PERFORMING_ACTION);
}

void mech_searchlight_set(Mech *mech, bool enabled) {
  if (enabled) {
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_SLITE_ON);
    mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_SLITE_LIT);
  } else {
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_SLITE_ON);
    mech_crit_status_clear(&mech->rd.critstatus, MECH_CRIT_STATUS_SLITE_LIT);
  }
}

void mech_searchlight_destroy(Mech *mech) {
  mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_SLITE_DEST);
  mech_status2_clear(&mech->rd.status2, MECH_STATUS2_SLITE_ON);
}

void mech_stealth_armor_active_set(Mech *mech, bool active) {
  if (active)
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_STH_ARMOR_ON);
  else
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_STH_ARMOR_ON);
}

void mech_null_signature_active_set(Mech *mech, bool active) {
  if (active)
    mech_status2_set(&mech->rd.status2, MECH_STATUS2_NULLSIGSYS_ON);
  else
    mech_status2_clear(&mech->rd.status2, MECH_STATUS2_NULLSIGSYS_ON);
}
