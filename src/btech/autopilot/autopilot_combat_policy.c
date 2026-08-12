/* Implements pure autonomous combat policy. */

#include "autopilot_combat_policy_api.h"

AutopilotWeaponDecision
autopilot_weapon_evaluate(const AutopilotWeaponSituation *situation) {
  AutopilotWeaponDecision decision = {.fire = false,
                                      .reason = AUTOPILOT_WEAPON_READY,
                                      .heat_after_fire =
                                          situation->projected_heat};
  if (!situation->functional) {
    decision.reason = AUTOPILOT_WEAPON_NONFUNCTIONAL;
  } else if (situation->recycling) {
    decision.reason = AUTOPILOT_WEAPON_RECYCLING;
  } else if (situation->defensive) {
    decision.reason = AUTOPILOT_WEAPON_DEFENSIVE;
  } else if (!situation->ammunition_compatible) {
    decision.reason = AUTOPILOT_WEAPON_INCOMPATIBLE_AMMO;
  } else if (situation->ammunition_required && situation->ammunition <= 0) {
    decision.reason = AUTOPILOT_WEAPON_NO_AMMO;
  } else if (!situation->in_arc) {
    decision.reason = AUTOPILOT_WEAPON_OUT_OF_ARC;
  } else if (situation->heat_limited && situation->projected_heat +
                                                (float)situation->weapon_heat -
                                                situation->heat_dissipation >
                                            situation->maximum_heat) {
    decision.reason = AUTOPILOT_WEAPON_OVERHEAT;
  } else {
    decision.fire = true;
    decision.heat_after_fire =
        situation->projected_heat + (float)situation->weapon_heat;
  }
  return decision;
}

int autopilot_weapon_score(const AutopilotWeaponScoreSituation *situation) {
  if (situation->range < 0 || situation->range >= situation->long_range)
    return 0;
  int range_score = 500;
  if (situation->range >= situation->medium_range)
    range_score = 215;
  else if (situation->range >= situation->short_range)
    range_score = 390;
  float minimum_range_score = 0.0F;
  if (situation->range < situation->minimum_range) {
    const int DELTA = situation->minimum_range - situation->range;
    minimum_range_score =
        (-12.5F * (float)(DELTA * DELTA)) - (25.0F * (float)DELTA);
  }
  return range_score + (50 * situation->damage) - (25 * situation->heat) + 250 +
         (int)minimum_range_score;
}

AutopilotPhysicalSide autopilot_physical_choose_leg(bool right_available,
                                                    int right_penalty,
                                                    bool left_available,
                                                    int left_penalty) {
  if (!right_available && !left_available)
    return AUTOPILOT_PHYSICAL_NONE;
  if (right_available && (!left_available || right_penalty <= left_penalty))
    return AUTOPILOT_PHYSICAL_RIGHT;
  return AUTOPILOT_PHYSICAL_LEFT;
}
