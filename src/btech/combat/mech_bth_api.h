
/* p.mech.bth.h */

#pragma once

#include "mux/server/platform.h"

int mech_normal_to_hit_calculate(Mech *mech, BattleMap *mech_map, int section,
                                 int critical, int weapindx, float range,
                                 Mech *target, int indirectFire, DbRef *c3Ref);
int mech_artillery_to_hit_calculate(Mech *mech, int section, int weapindx,
                                    int indirect, float range);
typedef enum WeaponRangeBracket : int {
  RANGE_SHORT = 0,
  RANGE_MED = 1,
  RANGE_LONG = 2,
  RANGE_EXTREME = 3,
  RANGE_TOFAR = 4,
  RANGE_NOWATER = 5,
} WeaponRangeBracket;

WeaponRangeBracket mech_range_to_hit_calculate(Mech *mech, Mech *target,
                                               int section, int weapindx,
                                               float frange, int firemode,
                                               int ammomode, int *wBTH);
WeaponRangeBracket mech_c3_range_to_hit_calculate(Mech *mech, Mech *target,
                                                  int section, int weapindx,
                                                  float realRange,
                                                  float c3Range, int mode,
                                                  int *wBTH);
int mech_attacker_movement_modifier(Mech *mech);
int mech_target_movement_modifier(Mech *mech, Mech *target, float range);

static_assert(RANGE_SHORT == 0 && RANGE_NOWATER == 5);
