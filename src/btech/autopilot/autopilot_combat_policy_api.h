/* Pure combat decisions for autonomous units. */

#pragma once

#include <stdbool.h>

typedef enum AutopilotWeaponSkipReason {
  AUTOPILOT_WEAPON_READY,
  AUTOPILOT_WEAPON_NONFUNCTIONAL,
  AUTOPILOT_WEAPON_RECYCLING,
  AUTOPILOT_WEAPON_DEFENSIVE,
  AUTOPILOT_WEAPON_INCOMPATIBLE_AMMO,
  AUTOPILOT_WEAPON_NO_AMMO,
  AUTOPILOT_WEAPON_OVERHEAT,
  AUTOPILOT_WEAPON_OUT_OF_ARC,
} AutopilotWeaponSkipReason;

typedef struct AutopilotWeaponSituation {
  bool functional;
  bool recycling;
  bool defensive;
  bool ammunition_required;
  int ammunition;
  bool ammunition_compatible;
  bool in_arc;
  bool heat_limited;
  float projected_heat;
  float heat_dissipation;
  int weapon_heat;
  float maximum_heat;
} AutopilotWeaponSituation;

typedef struct AutopilotWeaponDecision {
  bool fire;
  AutopilotWeaponSkipReason reason;
  float heat_after_fire;
} AutopilotWeaponDecision;

typedef struct AutopilotWeaponScoreSituation {
  int range;
  int minimum_range;
  int short_range;
  int medium_range;
  int long_range;
  int damage;
  int heat;
} AutopilotWeaponScoreSituation;

typedef enum AutopilotPhysicalSide {
  AUTOPILOT_PHYSICAL_NONE,
  AUTOPILOT_PHYSICAL_RIGHT,
  AUTOPILOT_PHYSICAL_LEFT,
} AutopilotPhysicalSide;

AutopilotWeaponDecision
autopilot_weapon_evaluate(const AutopilotWeaponSituation *situation);
int autopilot_weapon_score(const AutopilotWeaponScoreSituation *situation);
AutopilotPhysicalSide autopilot_physical_choose_leg(bool right_available,
                                                    int right_penalty,
                                                    bool left_available,
                                                    int left_penalty);
