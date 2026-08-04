#include "mech_specification_api.h"

#include <math.h>

#include "mech_internal.h"
#include "mech_status_types.h"

int mech_movement_type(const Mech *mech) { return mech->ud.move; }

int mech_tonnage(const Mech *mech) { return mech->ud.tons; }

int mech_engine_rating(const Mech *mech) {
  if (mech->rd.erat > 0)
    return mech->rd.erat;
  return (int)rint((2 * mech->ud.maxspeed / KPH_PER_MP) / 3) * mech->ud.tons;
}

float mech_jump_speed(const Mech *mech) { return mech->rd.jumpspeed; }

int mech_heat_sink_count(const Mech *mech) { return mech->ud.numsinks; }

int mech_technology_flags(const Mech *mech) { return mech->rd.specials; }

int mech_technology_flags_secondary(const Mech *mech) {
  return mech->rd.specials2;
}

float mech_current_speed(const Mech *mech) { return mech->rd.speed; }

float mech_maximum_speed(const Mech *mech) { return mech->ud.maxspeed; }

bool mech_is_flying_type(const Mech *mech) {
  return mech->ud.type == CLASS_AERO || mech->ud.type == CLASS_DS ||
         mech->ud.type == CLASS_SPHEROID_DS || mech->ud.move == MOVE_VTOL;
}

bool mech_is_omni(const Mech *mech) {
  return mech->rd.specials2 & OMNIMECH_TECH;
}

int mech_fuel(const Mech *mech) { return mech->ud.fuel; }

int mech_original_fuel(const Mech *mech) { return mech->ud.fuel_orig; }

void mech_maximum_fuel_set(Mech *mech, int fuel) { mech->rd.maxfuel = fuel; }

void mech_cargo_weight_set(Mech *mech, int weight) {
  mech->rd.cargo_weight = weight;
  mech->rd.critstatus &= ~LOAD_OK;
}
