#include "autopilot_movement_policy_api.h"

#include <math.h>

static bool nearly_equal(float left, float right) {
  return fabsf(left - right) < 0.0001F;
}

static int test_heading_difference(void) {
  if (autopilot_heading_difference(350, 10) != 20 ||
      autopilot_heading_difference(10, 350) != 20 ||
      autopilot_heading_difference(0, 180) != 180 ||
      autopilot_heading_difference(180, 0) != 180 ||
      autopilot_heading_difference(0, 30) != 30 ||
      autopilot_heading_difference(0, 31) != 31 ||
      autopilot_heading_difference(90, 90) != 0)
    return 1;
  /* Headings outside a single turn still reduce to the wrapped separation. */
  if (autopilot_heading_difference(370, 10) != 0 ||
      autopilot_heading_difference(-10, 350) != 0)
    return 2;
  return 0;
}

static int test_approach(void) {
  AutopilotApproachSituation situation = {
      .range = 2.01F, .bearing = 90, .heading = 90};
  if (autopilot_approach_evaluate(&situation).action !=
      AUTOPILOT_APPROACH_KEEP_MOVING)
    return 1;

  /* A distant goal keeps the unit moving even when it is standing on it. */
  situation.at_target = true;
  if (autopilot_approach_evaluate(&situation).action !=
      AUTOPILOT_APPROACH_KEEP_MOVING)
    return 2;
  situation.at_target = false;

  situation.range = -1.0F;
  situation.heading = 121;
  if (autopilot_approach_evaluate(&situation).action !=
      AUTOPILOT_APPROACH_TURN_AND_STOP)
    return 3;

  situation.heading = 120;
  situation.at_target = true;
  if (autopilot_approach_evaluate(&situation).action != AUTOPILOT_APPROACH_STOP)
    return 4;

  /* Negative ranges clamp to zero rather than feeding the slowdown ratio. */
  situation.at_target = false;
  AutopilotApproachDecision decision = autopilot_approach_evaluate(&situation);
  if (decision.action != AUTOPILOT_APPROACH_SLOW ||
      !nearly_equal(decision.speed_ratio, 0.4F))
    return 5;

  situation.bearing = 350;
  situation.heading = 10;
  if (autopilot_approach_evaluate(&situation).action != AUTOPILOT_APPROACH_SLOW)
    return 6;
  situation.heading = 319;
  if (autopilot_approach_evaluate(&situation).action !=
      AUTOPILOT_APPROACH_TURN_AND_STOP)
    return 7;

  situation.range = 2.0F;
  decision = autopilot_approach_evaluate(&situation);
  if (decision.action != AUTOPILOT_APPROACH_TURN_AND_STOP)
    return 8;
  situation.heading = 350;
  decision = autopilot_approach_evaluate(&situation);
  if (decision.action != AUTOPILOT_APPROACH_SLOW ||
      !nearly_equal(decision.speed_ratio, 1.4F))
    return 9;

  situation.range = 1.0F;
  decision = autopilot_approach_evaluate(&situation);
  return decision.action != AUTOPILOT_APPROACH_SLOW ||
         !nearly_equal(decision.speed_ratio, 0.9F);
}

static int test_cruise(void) {
  AutopilotCruiseSituation situation = {
      .bearing = 90, .heading = 90, .desired_speed = 1.99F};
  if (!autopilot_cruise_should_accelerate(&situation))
    return 1;

  situation.desired_speed = 2.0F;
  if (autopilot_cruise_should_accelerate(&situation))
    return 2;

  situation.desired_speed = 0.0F;
  situation.heading = 121;
  if (autopilot_cruise_should_accelerate(&situation))
    return 3;

  situation.bearing = 350;
  situation.heading = 10;
  if (!autopilot_cruise_should_accelerate(&situation))
    return 4;
  situation.heading = 319;
  if (autopilot_cruise_should_accelerate(&situation))
    return 5;

  /* A negative bearing means "no bearing to hold", so neither the speed nor
   * the heading gate applies. */
  situation.bearing = -1;
  situation.desired_speed = 40.0F;
  if (!autopilot_cruise_should_accelerate(&situation))
    return 6;

  situation.at_target = true;
  if (autopilot_cruise_should_accelerate(&situation))
    return 7;
  situation.bearing = 90;
  situation.heading = 90;
  situation.desired_speed = 0.0F;
  return autopilot_cruise_should_accelerate(&situation);
}

static int test_cruise_speed_ratio(void) {
  return !nearly_equal(autopilot_cruise_speed_ratio(false), 1.0F) ||
         !nearly_equal(autopilot_cruise_speed_ratio(true), 2.0F / 3.0F);
}

int main(void) {
  const int HEADING = test_heading_difference();
  if (HEADING)
    return 10 + HEADING;
  const int APPROACH = test_approach();
  if (APPROACH)
    return 20 + APPROACH;
  const int CRUISE = test_cruise();
  if (CRUISE)
    return 40 + CRUISE;
  return test_cruise_speed_ratio() ? 60 : 0;
}
