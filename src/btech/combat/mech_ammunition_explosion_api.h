#pragma once

#include "mech_api_types.h"

void mech_ammunition_explode(Mech *attacker, Mech *mech, int ammunition_section,
                             int ammunition_critical, int damage);
