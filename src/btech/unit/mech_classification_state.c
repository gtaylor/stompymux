#include "mech_classification_api.h"

#include "mech_internal.h"

int mech_class(const Mech *mech) { return mech->ud.type; }

void mech_class_set(Mech *mech, int unit_class) { mech->ud.type = unit_class; }

bool mech_is_dropship(const Mech *mech) {
  return mech->ud.type == CLASS_DS || mech->ud.type == CLASS_SPHEROID_DS;
}

bool mech_is_aerospace_unit(const Mech *mech) {
  return mech->ud.type == CLASS_AERO || mech_is_dropship(mech);
}

bool mech_is_quad(const Mech *mech) { return mech->ud.move == MOVE_QUAD; }

int mech_team(const Mech *mech) { return mech->pd.team; }

void mech_team_set(Mech *mech, int team) { mech->pd.team = team; }
