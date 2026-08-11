#include "autopilot_combat_policy_api.h"

static AutopilotWeaponSituation ready_weapon(void) {
  return (AutopilotWeaponSituation){.functional = true,
                                    .ammunition_compatible = true,
                                    .in_arc = true,
                                    .heat_limited = true,
                                    .projected_heat = 4.0F,
                                    .heat_dissipation = 2.0F,
                                    .weapon_heat = 4,
                                    .maximum_heat = 6.0F};
}

static int expect_reason(AutopilotWeaponSituation situation,
                         AutopilotWeaponSkipReason reason) {
  AutopilotWeaponDecision decision = autopilot_weapon_evaluate(&situation);
  return decision.fire || decision.reason != reason;
}

int main(void) {
  AutopilotWeaponScoreSituation score = {.range = 2,
                                         .minimum_range = 0,
                                         .short_range = 5,
                                         .medium_range = 10,
                                         .long_range = 15,
                                         .damage = 10,
                                         .heat = 4};
  if (autopilot_weapon_score(&score) != 1150)
    return 1;
  score.range = 5;
  if (autopilot_weapon_score(&score) != 1040)
    return 2;
  score.range = 10;
  if (autopilot_weapon_score(&score) != 865)
    return 3;
  score.range = 15;
  if (autopilot_weapon_score(&score) != 0)
    return 4;
  score.range = 1;
  score.minimum_range = 5;
  if (autopilot_weapon_score(&score) >= 1150)
    return 5;
  AutopilotWeaponSituation situation = ready_weapon();
  AutopilotWeaponDecision decision = autopilot_weapon_evaluate(&situation);
  if (!decision.fire || decision.heat_after_fire != 8.0F)
    return 6;
  situation.functional = false;
  if (expect_reason(situation, AUTOPILOT_WEAPON_NONFUNCTIONAL))
    return 7;
  situation = ready_weapon();
  situation.recycling = true;
  if (expect_reason(situation, AUTOPILOT_WEAPON_RECYCLING))
    return 8;
  situation = ready_weapon();
  situation.defensive = true;
  if (expect_reason(situation, AUTOPILOT_WEAPON_DEFENSIVE))
    return 9;
  situation = ready_weapon();
  situation.ammunition_compatible = false;
  if (expect_reason(situation, AUTOPILOT_WEAPON_INCOMPATIBLE_AMMO))
    return 10;
  situation = ready_weapon();
  situation.ammunition_required = true;
  if (expect_reason(situation, AUTOPILOT_WEAPON_NO_AMMO))
    return 11;
  situation.ammunition = 1;
  if (!autopilot_weapon_evaluate(&situation).fire)
    return 12;
  situation = ready_weapon();
  situation.projected_heat = 4.1F;
  if (expect_reason(situation, AUTOPILOT_WEAPON_OVERHEAT))
    return 13;
  situation = ready_weapon();
  situation.in_arc = false;
  if (expect_reason(situation, AUTOPILOT_WEAPON_OUT_OF_ARC))
    return 14;
  if (autopilot_physical_choose_leg(true, 2, true, 3) !=
          AUTOPILOT_PHYSICAL_RIGHT ||
      autopilot_physical_choose_leg(true, 4, true, 1) !=
          AUTOPILOT_PHYSICAL_LEFT ||
      autopilot_physical_choose_leg(false, 0, true, 9) !=
          AUTOPILOT_PHYSICAL_LEFT ||
      autopilot_physical_choose_leg(false, 0, false, 0) !=
          AUTOPILOT_PHYSICAL_NONE)
    return 15;
  return 0;
}
