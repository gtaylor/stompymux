#include "mech_heat_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

float mech_excess_heat(const Mech *mech) { return mech->rd.heat; }

float mech_heat_production(const Mech *mech) { return mech->rd.plus_heat; }

float mech_heat_dissipation(const Mech *mech) { return mech->rd.minus_heat; }

float mech_weapon_heat(const Mech *mech) { return mech->rd.weapheat; }

float mech_active_heat_sinks(const Mech *mech) {
  return mech->ud.numsinks - mech->rd.disabled_hs;
}

bool mech_uses_heat(const Mech *mech) {
  return mech->ud.type == CLASS_MECH || mech->ud.type == CLASS_AERO ||
         mech->ud.type == CLASS_DS || mech->ud.type == CLASS_SPHEROID_DS;
}

float mech_added_heat(const Mech *mech) { return mech->rd.plus_heat; }
