#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

typedef struct MechConditionSummary {
  bool combat_safe;
  bool fortified;
  bool weapons_hold;
  bool hull_down;
  bool dug_in;
  bool digging;
  bool staggering;
  bool searchlight_destroyed;
  bool illuminated;
  bool hidden;
  bool dodging;
  bool evading;
  bool sprinting;
  bool stunned;
  bool ecm_protected;
  bool angel_ecm_protected;
  bool angel_ecm_disturbed;
  bool ecm_countered;
  bool ecm_destroyed;
  bool ecm_enabled;
  bool ecm_active;
  bool eccm_enabled;
  bool angel_ecm_destroyed;
  bool angel_ecm_enabled;
  bool angel_ecm_active;
  bool angel_eccm_enabled;
  bool personal_ecm_enabled;
  bool personal_ecm_active;
  bool personal_eccm_enabled;
  bool stealth_armor_active;
  bool null_signature_active;
  bool null_signature_destroyed;
  bool c3_destroyed;
  bool c3i_destroyed;
  bool turret_auto_turn;
  bool arms_flipped;
  bool targeting_computer_destroyed;
  bool ams_enabled;
  bool supercharger_enabled;
  bool masc_enabled;
  bool player_killer;
  bool tight_turn_mode;
  bool dfa_attacking;
  bool turret_jammed;
  bool turret_locked;
  bool torso_right;
  bool torso_left;
  DbRef swarm_target;
  int supercharger_counter;
  int masc_counter;
} MechConditionSummary;

typedef enum MechTorsoTwist {
  MECH_TORSO_CENTER,
  MECH_TORSO_LEFT,
  MECH_TORSO_RIGHT,
} MechTorsoTwist;

MechConditionSummary mech_condition_summary(const Mech *mech);
void mech_torso_twist_set(Mech *mech, MechTorsoTwist twist);
void mech_arms_center(Mech *mech);
void mech_arms_flip(Mech *mech);
void mech_partial_cover_set(Mech *mech, bool covered);
