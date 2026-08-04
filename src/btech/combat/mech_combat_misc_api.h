
/* p.mech.combat.misc.h */

#pragma once

#include "mux/server/platform.h"

void decrement_ammunition(Mech *mech, int weapindx, int section, int critical,
                          int ammoLoc, int ammoCrit, int ammoLoc1,
                          int ammoCrit1, int wGattlingShots);
void ammo_expedinture_check(Mech *mech, int weapindx, int ns);
void heat_effect(Mech *mech, Mech *tempMech, int heatdam, int fromInferno);
void Inferno_Hit(Mech *mech, Mech *hitMech, int missiles, int LOS);
void Plasma_Hit(Mech *mech, Mech *hitMech, int LOS);
void KillMechContentsIfIC(Mech *mech);
void DestroyMech(Mech *target, Mech *mech, int showboom, const char *reason);
char *short_hextarget(Mech *mech);
