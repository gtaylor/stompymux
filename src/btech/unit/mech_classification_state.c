#include "mech_classification_api.h"

#include "mech_internal.h"

int mech_class(const Mech *mech) { return mech->ud.type; }

int mech_team(const Mech *mech) { return mech->pd.team; }

void mech_team_set(Mech *mech, int team) { mech->pd.team = team; }
