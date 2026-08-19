#include "autopilot_combat_policy_api.h"

#include <math.h>

static bool nearly_equal(float left, float right) {
  return fabsf(left - right) < 0.0001F;
}

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

/*
 * A skipped weapon must report the expected reason and must leave the heat
 * budget exactly as it found it, so a caller can keep evaluating the rest of
 * the weapon list against the same projection.
 */
static int expect_reason(AutopilotWeaponSituation situation,
                         AutopilotWeaponSkipReason reason) {
  AutopilotWeaponDecision decision = autopilot_weapon_evaluate(&situation);
  return decision.fire || decision.reason != reason ||
         !nearly_equal(decision.heat_after_fire, situation.projected_heat);
}

static int test_heat_sequence(void) {
  AutopilotWeaponSituation first = ready_weapon();
  AutopilotWeaponDecision decision = autopilot_weapon_evaluate(&first);
  if (!decision.fire || !nearly_equal(decision.heat_after_fire, 8.0F))
    return 1;

  AutopilotWeaponSituation second = ready_weapon();
  second.projected_heat = decision.heat_after_fire;
  if (expect_reason(second, AUTOPILOT_WEAPON_OVERHEAT))
    return 2;

  first.projected_heat = 6.0F;
  first.heat_dissipation = 4.0F;
  first.maximum_heat = 6.0F;
  return !autopilot_weapon_evaluate(&first).fire;
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
  score.range = -1;
  if (autopilot_weapon_score(&score) != 0)
    return 5;
  /* Minimum-range brackets subtract 12.5 per squared step plus 25 per step:
   * four steps inside the minimum cost 200 + 100 off the 1150 baseline. A
   * single step costs 37.5, which truncates toward zero to 37. */
  score.range = 1;
  score.minimum_range = 5;
  if (autopilot_weapon_score(&score) != 850)
    return 6;
  score.minimum_range = 2;
  if (autopilot_weapon_score(&score) != 1113)
    return 22;
  score.minimum_range = 0;
  score.damage = 12;
  score.heat = 6;
  if (autopilot_weapon_score(&score) != 1200)
    return 23;
  score.damage = 10;
  score.heat = 4;
  AutopilotWeaponSituation situation = ready_weapon();
  AutopilotWeaponDecision decision = autopilot_weapon_evaluate(&situation);
  if (!decision.fire || !nearly_equal(decision.heat_after_fire, 8.0F))
    return 7;
  situation.functional = false;
  if (expect_reason(situation, AUTOPILOT_WEAPON_NONFUNCTIONAL))
    return 8;
  situation = ready_weapon();
  situation.recycling = true;
  if (expect_reason(situation, AUTOPILOT_WEAPON_RECYCLING))
    return 9;
  situation = ready_weapon();
  situation.defensive = true;
  if (expect_reason(situation, AUTOPILOT_WEAPON_DEFENSIVE))
    return 10;
  situation = ready_weapon();
  situation.ammunition_compatible = false;
  if (expect_reason(situation, AUTOPILOT_WEAPON_INCOMPATIBLE_AMMO))
    return 11;
  situation = ready_weapon();
  situation.ammunition_required = true;
  if (expect_reason(situation, AUTOPILOT_WEAPON_NO_AMMO))
    return 12;
  situation.ammunition = 1;
  if (!autopilot_weapon_evaluate(&situation).fire)
    return 13;
  situation = ready_weapon();
  situation.projected_heat = 4.1F;
  if (expect_reason(situation, AUTOPILOT_WEAPON_OVERHEAT))
    return 14;
  situation = ready_weapon();
  situation.in_arc = false;
  if (expect_reason(situation, AUTOPILOT_WEAPON_OUT_OF_ARC))
    return 15;
  situation = ready_weapon();
  situation.projected_heat = 4.0F;
  if (!autopilot_weapon_evaluate(&situation).fire)
    return 16;
  situation.projected_heat = 100.0F;
  situation.heat_limited = false;
  if (!autopilot_weapon_evaluate(&situation).fire)
    return 17;
  if (autopilot_physical_choose_leg(true, 2, true, 3) !=
          AUTOPILOT_PHYSICAL_RIGHT ||
      autopilot_physical_choose_leg(true, 4, true, 1) !=
          AUTOPILOT_PHYSICAL_LEFT ||
      autopilot_physical_choose_leg(false, 0, true, 9) !=
          AUTOPILOT_PHYSICAL_LEFT ||
      autopilot_physical_choose_leg(false, 0, false, 0) !=
          AUTOPILOT_PHYSICAL_NONE)
    return 18;
  if (autopilot_physical_choose_leg(true, 3, true, 3) !=
      AUTOPILOT_PHYSICAL_RIGHT)
    return 19;
  if (autopilot_physical_choose_punch(true, true) != AUTOPILOT_PHYSICAL_BOTH ||
      autopilot_physical_choose_punch(true, false) !=
          AUTOPILOT_PHYSICAL_RIGHT ||
      autopilot_physical_choose_punch(false, true) != AUTOPILOT_PHYSICAL_LEFT ||
      autopilot_physical_choose_punch(false, false) != AUTOPILOT_PHYSICAL_NONE)
    return 20;
  return test_heat_sequence() ? 21 : 0;
}
