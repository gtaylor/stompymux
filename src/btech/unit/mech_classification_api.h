#pragma once

#include "mech_api_types.h"

int mech_class(const Mech *mech);
int mech_team(const Mech *mech);
void mech_team_set(Mech *mech, int team);
