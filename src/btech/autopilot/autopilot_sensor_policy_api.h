/* Pure sensor selection for autonomous units. */

#pragma once

#include <stdbool.h>

typedef struct AutopilotSensorSituation {
  bool has_target;
  int target_range;
  int target_tonnage;
  bool target_flying;
  bool target_landed;
  bool has_beagle_probe;
  bool has_bloodhound_probe;
  int preferred_visual_sensor;
  int effective_visibility;
} AutopilotSensorSituation;

typedef struct AutopilotSensorSelection {
  int primary;
  int secondary;
} AutopilotSensorSelection;

int autopilot_searchlight_classify(bool active, bool in_arc,
                                   bool line_of_sight_blocked);
int autopilot_visual_sensor_select(bool observer_lit, bool target_lit,
                                   int map_light,
                                   int searchlight_classification);
AutopilotSensorSelection
autopilot_sensor_select(const AutopilotSensorSituation *situation);
