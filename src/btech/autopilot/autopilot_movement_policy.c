/* Implements pure movement decisions for autonomous units. */

#include "autopilot_movement_policy_api.h"

#include <stdlib.h>

int autopilot_heading_difference(int bearing, int heading) {
  int difference = (bearing - heading) % 360;
  if (difference < -180)
    difference += 360;
  else if (difference > 180)
    difference -= 360;
  return abs(difference);
}

AutopilotApproachDecision
autopilot_approach_evaluate(const AutopilotApproachSituation *situation) {
  float range = situation->range;
  if (range < 0.0F)
    range = 0.0F;
  if (range > 2.0F)
    return (AutopilotApproachDecision){.action =
                                           AUTOPILOT_APPROACH_KEEP_MOVING};
  if (autopilot_heading_difference(situation->bearing, situation->heading) > 30)
    return (AutopilotApproachDecision){.action =
                                           AUTOPILOT_APPROACH_TURN_AND_STOP};
  if (situation->at_target)
    return (AutopilotApproachDecision){.action = AUTOPILOT_APPROACH_STOP};
  return (AutopilotApproachDecision){.action = AUTOPILOT_APPROACH_SLOW,
                                     .speed_ratio = 0.4F + (range / 2.0F)};
}

bool autopilot_cruise_should_accelerate(
    const AutopilotCruiseSituation *situation) {
  if (situation->at_target)
    return false;
  if (situation->bearing < 0)
    return true;
  return (situation->desired_speed < 2.0F &&
          autopilot_heading_difference(situation->bearing,
                                       situation->heading) <= 30) != 0;
}

float autopilot_cruise_speed_ratio(bool in_water) {
  return in_water ? 2.0F / 3.0F : 1.0F;
}
