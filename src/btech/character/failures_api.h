/* Declares the BattleTech failures API. */

#pragma once

#include "mux/server/platform.h"

#include "mech_api_types.h"

const char *mech_part_brand_name(int type, int level);
void mech_generic_failure_check(Mech *mech, int type, int *result,
                                int *modifier);
void mech_weapon_failure_check(Mech *mech, int weapon_number, int weapon_type,
                               int section, int critical, int *modifier,
                               int *type);
