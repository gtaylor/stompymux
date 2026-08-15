#include "mech_classification_api.h"

#include "mech_internal.h"
#include "section_types.h"

UnitClass mech_class(const Mech *mech) { return (UnitClass)mech->ud.type; }

void mech_class_set(Mech *mech, UnitClass unit_class) {
  mech->ud.type = (char)unit_class;
}

bool mech_is_dropship(const Mech *mech) {
  return (mech->ud.type == CLASS_DS || mech->ud.type == CLASS_SPHEROID_DS) != 0;
}

bool mech_is_aerospace_unit(const Mech *mech) {
  return (mech->ud.type == CLASS_AERO || mech_is_dropship(mech)) != 0;
}

bool mech_is_quad(const Mech *mech) { return mech->ud.move == MOVE_QUAD; }

bool mech_is_biped(const Mech *mech) { return mech->ud.move == MOVE_BIPED; }

bool mech_is_rolling_aerospace_unit(const Mech *mech) {
  return (mech->ud.type == CLASS_AERO || mech->ud.type == CLASS_DS) != 0;
}

int mech_team(const Mech *mech) { return mech->pd.team; }

void mech_team_set(Mech *mech, int team) { mech->pd.team = team; }
