#pragma once

#include "autopilot.h"

int auto_targets_callback(void *key, void *data, int depth, void *arg);
int auto_generic_compare(void *a, void *b, void *token);
AutopilotTarget *auto_create_target_node(int target_score, DbRef target_dbref);
int auto_calc_target_score(Autopilot *autopilot, Mech *mech, Mech *target,
                           BattleMap *map);
void autogun_physical_attack(Autopilot *autopilot, Mech *mech, BattleMap *map,
                             Mech *target);
bool autogun_chase_target(Autopilot *autopilot, Mech *mech, BattleMap *map,
                          Mech *target);
