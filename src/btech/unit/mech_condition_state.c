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
      .fortified = status2 & FORTIFIED,
      .weapons_hold = status2 & WEAPONS_HOLD,
      .hull_down = status & HULLDOWN,
      .dug_in = tank_critical_status & DUG_IN,
      .digging = tank_critical_status & DIGGING_IN,
      .staggering = mech->rd.staggerDamage / 20 > 0,
      .searchlight_destroyed = critical_status & SLITE_DEST,
      .illuminated = critical_status & SLITE_LIT,
      .hidden = critical_status & HIDDEN,
      .dodging = status2 & DODGING,
      .evading = status2 & EVADING,
      .sprinting = status2 & SPRINTING,
      .stunned = (tank_critical_status & CREW_STUNNED) ||
                 (critical_status & MECH_STUNNED),
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
      .torso_right = status & TORSO_RIGHT,
      .torso_left = status & TORSO_LEFT,
      .swarm_target = mech->rd.swarming,
      .supercharger_counter = mech->rd.scharge_value,
      .masc_counter = mech->rd.masc_value,
  };
}

void mech_torso_twist_set(Mech *mech, MechTorsoTwist twist) {
  mech->rd.status &= ~(TORSO_RIGHT | TORSO_LEFT);
  if (twist == MECH_TORSO_LEFT)
    mech->rd.status |= TORSO_LEFT;
  else if (twist == MECH_TORSO_RIGHT)
    mech->rd.status |= TORSO_RIGHT;
}

void mech_arms_center(Mech *mech) { mech->rd.status &= ~FLIPPED_ARMS; }
