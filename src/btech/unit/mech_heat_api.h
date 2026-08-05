#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

float mech_excess_heat(const Mech *mech);
float mech_heat_production(const Mech *mech);
float mech_heat_dissipation(const Mech *mech);
float mech_weapon_heat(const Mech *mech);
float mech_active_heat_sinks(const Mech *mech);
bool mech_uses_heat(const Mech *mech);
float mech_added_heat(const Mech *mech);
