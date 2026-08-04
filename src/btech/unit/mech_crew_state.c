#include "mech_crew_api.h"

#include "btech/context.h"
#include "mech_internal.h"

DbRef mech_pilot_dbref(const Mech *mech) { return mech->pd.pilot; }

void mech_pilot_dbref_set(Mech *mech, DbRef pilot) { mech->pd.pilot = pilot; }

DbRef mech_gunner_dbref(const Mech *mech) {
  DbRef override = mech->xcode.context->combat_overrides.pilot;
  return override > 0 ? override : mech_pilot_dbref(mech);
}

int mech_pilot_status(const Mech *mech) { return mech->pd.pilotstatus; }

void mech_pilot_status_set(Mech *mech, int status) {
  mech->pd.pilotstatus = status;
}

void mech_pilot_status_add(Mech *mech, int damage) {
  mech->pd.pilotstatus += damage;
}

int mech_perception_target(const Mech *mech) { return mech->rd.per; }

void mech_perception_target_set(Mech *mech, int target) {
  mech->rd.per = target;
}
