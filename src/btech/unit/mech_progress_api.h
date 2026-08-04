#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

bool mech_piloting_position_mark_changed(Mech *mech);
int mech_battle_value(const Mech *mech);
float mech_experience_modifier(const Mech *mech);
