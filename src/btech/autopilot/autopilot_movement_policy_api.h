/* Movement decisions shared by autopilot navigation and its tests. */

#pragma once

#include <stdbool.h>

typedef enum AutopilotApproachAction : int {
  AUTOPILOT_APPROACH_KEEP_MOVING,
  AUTOPILOT_APPROACH_TURN_AND_STOP,
  AUTOPILOT_APPROACH_STOP,
  AUTOPILOT_APPROACH_SLOW,
} AutopilotApproachAction;

typedef struct AutopilotApproachSituation {
  float range;
  int bearing;
  int heading;
  bool at_target;
} AutopilotApproachSituation;

typedef struct AutopilotApproachDecision {
  AutopilotApproachAction action;
  float speed_ratio;
} AutopilotApproachDecision;

typedef struct AutopilotCruiseSituation {
  bool at_target;
  int bearing;
  int heading;
  float desired_speed;
} AutopilotCruiseSituation;

/**
 * Return the smallest angular separation between two compass headings.
 *
 * Both arguments are degrees; the result is the wrapped separation in
 * [0, 180], so headings either side of north compare as neighbours rather
 * than as opposites.
 */
int autopilot_heading_difference(int bearing, int heading);
/** Decide whether an autopilot should continue, turn, stop, or slow near a
 * goal. */
AutopilotApproachDecision
autopilot_approach_evaluate(const AutopilotApproachSituation *situation);
/**
 * Decide whether an autopilot should accelerate while travelling toward a
 * goal.
 *
 * The decision deliberately excludes terrain so callers only pay for a map
 * lookup once acceleration is actually wanted; feed the result of that lookup
 * to autopilot_cruise_speed_ratio().
 */
bool autopilot_cruise_should_accelerate(
    const AutopilotCruiseSituation *situation);
/** Fraction of its maximum speed an accelerating autopilot should request. */
float autopilot_cruise_speed_ratio(bool in_water);
