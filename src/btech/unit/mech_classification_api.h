#pragma once

#include "mech_api_types.h"
#include "section_types.h"

#include <stdbool.h>

UnitClass mech_class(const Mech *mech);
void mech_class_set(Mech *mech, UnitClass unit_class);
bool mech_is_dropship(const Mech *mech);
bool mech_is_aerospace_unit(const Mech *mech);
bool mech_is_quad(const Mech *mech);
bool mech_is_biped(const Mech *mech);
bool mech_is_rolling_aerospace_unit(const Mech *mech);
int mech_team(const Mech *mech);
void mech_team_set(Mech *mech, int team);
