#pragma once

#include "autopilot.h"

bool auto_targets_callback(const RedBlackTreeVisitCall *call);
int auto_generic_compare(const RedBlackTreeCompareCall *call);
typedef struct AutopilotTargetRequest {
  int score;
  DbRef target;
} AutopilotTargetRequest;
AutopilotTarget *auto_create_target_node(const AutopilotTargetRequest *request);
int auto_calc_target_score(Autopilot *autopilot, Mech *mech, Mech *target,
                           BattleMap *map);
void autogun_physical_attack(Autopilot *autopilot, Mech *mech, BattleMap *map,
                             Mech *target);
bool autogun_chase_target(Autopilot *autopilot, Mech *mech, BattleMap *map,
                          Mech *target);
