#pragma once

#include "mech_api_types.h"

void mech_reactor_explode(Mech *wounded, Mech *attacker);
void mech_parts_destroy(Mech *attacker, Mech *wounded, int hitloc, bool breach,
                        bool disable);
int mech_location_breach(Mech *attacker, Mech *mech, int hitloc);
int mech_location_maybe_breach(Mech *attacker, Mech *mech, int hitloc);
