#include "autopilot.h"
#include "autopilot_resume_api.h"

#include "btech_event.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_runtime_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"

void autopilot_resume(Autopilot *autopilot) {
  autopilot_gunning_resume(autopilot);
  if (autopilot->flags & AUTOPILOT_PILZOMBIE) {
    autopilot->flags &= (unsigned short)~AUTOPILOT_PILZOMBIE;
    autopilot_event_schedule(autopilot, EVENT_AUTOCOM, auto_com_event, 1, 0);
  }
}

void autopilot_resume_for_mech(Mech *mech) {
  DbRef autopilot_dbref = mech_autopilot_dbref(mech);
  if (autopilot_dbref <= 0)
    return;

  Autopilot *autopilot =
      btech_context_find_object(mech_context(mech), autopilot_dbref);
  if (autopilot)
    autopilot_resume(autopilot);
}
