#pragma once

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "btconfig.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_sensor.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/flags.h"
#include "mux/server/diagnostics.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"

int auto_targets_callback(void *key, void *data, int depth, void *arg);
int auto_generic_compare(void *a, void *b, void *token);
AutopilotTarget *auto_create_target_node(int target_score, DbRef target_dbref);
int auto_calc_target_score(Autopilot *autopilot, Mech *mech, Mech *target,
                           BattleMap *map);
void autogun_physical_attack(Autopilot *autopilot, Mech *mech, BattleMap *map,
                             Mech *target);
bool autogun_chase_target(Autopilot *autopilot, Mech *mech, BattleMap *map,
                          Mech *target);
