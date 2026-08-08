/* Bounds-checked access to autopilot weapon profiles. */

#pragma once

#include <stddef.h>

#include "mux/support/red_black_tree.h"

typedef struct Autopilot Autopilot;
typedef struct AutopilotWeapon AutopilotWeapon;

void autopilot_weapon_profiles_initialize(Autopilot *autopilot);
void autopilot_weapon_profiles_clear(Autopilot *autopilot);
RedBlackTree autopilot_weapon_profile_get(const Autopilot *autopilot,
                                          int range);
void autopilot_weapon_profile_set(Autopilot *autopilot, int range,
                                  RedBlackTree profile);
int *autopilot_weapon_range_score_key(AutopilotWeapon *weapon, int range);
