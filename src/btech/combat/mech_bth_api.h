
/* p.mech.bth.h */

#pragma once

#include "mux/server/platform.h"

int mech_normal_to_hit_calculate(Mech *mech, BattleMap *mech_map, int section,
                                 int critical, int weapindx, float range,
                                 Mech *target, int indirectFire, DbRef *c3Ref);
int mech_artillery_to_hit_calculate(Mech *mech, int section, int weapindx,
                                    int indirect, float range);
int mech_range_to_hit_calculate(Mech *mech, Mech *target, int section,
                                int weapindx, float frange, int firemode,
                                int ammomode, int *wBTH);
int mech_c3_range_to_hit_calculate(Mech *mech, Mech *target, int section,
                                   int weapindx, float realRange, float c3Range,
                                   int mode, int *wBTH);
int mech_attacker_movement_modifier(Mech *mech);
int mech_target_movement_modifier(Mech *mech, Mech *target, float range);

#define RANGE_SHORT 0
#define RANGE_MED 1
#define RANGE_LONG 2
#define RANGE_EXTREME 3
#define RANGE_TOFAR 4
#define RANGE_NOWATER 5
