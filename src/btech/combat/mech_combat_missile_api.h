
/* p.mech.combat.h */

#pragma once

#include "mux/server/platform.h"

void mech_missile_apply_hits(Mech *mech, Mech *target, int hitX, int hitY,
                             int isrear, int iscritical, int weapindx,
                             int fireMode, int ammoMode, int num_missiles_hit,
                             int damage, int salvo_size, int LOS, int bth,
                             int tIsSwarmAttack);
int mech_ams_intercept(Mech *mech, Mech *hitMech, int incoming, int type,
                       int ammoLoc, int ammoCrit, int LOS, int missilesDidHit);
int mech_ams_locate_defenses(Mech *target, int *AMStype, int *ammoLoc,
                             int *ammoCrit);
int mech_missile_hit_index(Mech *mech, Mech *hitMech, int weapindx,
                           int wSection, int wCritSlot, int glance);
int mech_missile_hit_target(Mech *mech, int weapindx, int wSection,
                            int wCritSlot, Mech *hitMech, int hitX, int hitY,
                            int LOS, int baseToHit, int roll, int incoming,
                            int tIsSwarmAttack, int player_roll);
void mech_swarm_missile_hit_target(Mech *mech, int weapindx, int wSection,
                                   int wCritSlot, Mech *hitMech, int LOS,
                                   int baseToHit, int roll, int incoming,
                                   int fof, int tIsSwarmAttack,
                                   int player_roll);
