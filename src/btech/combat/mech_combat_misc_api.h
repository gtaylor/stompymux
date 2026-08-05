
/* p.mech.combat.misc.h */

#pragma once

#include "mux/server/platform.h"

void mech_ammunition_decrement(Mech *mech, int weapindx, int section,
                               int critical, int ammoLoc, int ammoCrit,
                               int ammoLoc1, int ammoCrit1, int wGattlingShots);
void mech_ammunition_expenditure_check(Mech *mech, int weapindx, int ns);
void mech_heat_effect_apply(Mech *mech, Mech *tempMech, int heatdam,
                            bool fromInferno);
void mech_inferno_hit(Mech *mech, Mech *hitMech, int missiles, bool LOS);
void mech_plasma_hit(Mech *mech, Mech *hitMech, bool LOS);
void mech_contents_kill_if_in_character(Mech *mech);
void mech_destroy(Mech *target, Mech *mech, bool showboom, const char *reason);
const char *mech_hex_target_short_name(const Mech *mech);
