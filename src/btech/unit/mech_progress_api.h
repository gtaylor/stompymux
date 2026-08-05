#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

bool mech_piloting_position_mark_changed(Mech *mech);
int mech_battle_value(const Mech *mech);
void mech_battle_value_set(Mech *mech, int battle_value);
float mech_experience_modifier(const Mech *mech);
void mech_shot_result_record(Mech *mech, bool hit);
