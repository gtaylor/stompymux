#include "mech_runtime_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

bool mech_is_started(const Mech *mech) { return mech->rd.status & STARTED; }

DbRef mech_autopilot_dbref(const Mech *mech) { return mech->rd.autopilot_num; }
