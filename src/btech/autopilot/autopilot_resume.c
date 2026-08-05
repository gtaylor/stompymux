#include "autopilot.h"
#include "autopilot_resume_api.h"

#include "btech/context.h"
#include "mech_identity_api.h"
#include "mech_runtime_api.h"
#include "registry_api.h"

void autopilot_resume_for_mech(Mech *mech) {
  DbRef autopilot_dbref = mech_autopilot_dbref(mech);
  if (autopilot_dbref <= 0)
    return;

  Autopilot *autopilot =
      btech_context_find_object(mech_context(mech), autopilot_dbref);
  if (autopilot)
    UnZombifyAuto(autopilot);
}
