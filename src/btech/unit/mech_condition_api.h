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
  bool stealth_armor_active;
  bool null_signature_active;
  bool turret_auto_turn;
  bool torso_right;
  bool torso_left;
  DbRef swarm_target;
} MechConditionSummary;

MechConditionSummary mech_condition_summary(const Mech *mech);
