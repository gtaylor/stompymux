#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

bool mech_piloting_position_mark_changed(Mech *mech);
double mech_experience_modifier(const Mech *mech);
void mech_shot_result_record(Mech *mech, bool hit);
void mech_shots_fired_increment(Mech *mech);
int mech_hexes_walked_advance(Mech *mech);
