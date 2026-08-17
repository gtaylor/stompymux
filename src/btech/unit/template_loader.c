#include "mech_template_api.h"

#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "mech_crew_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mux/objects/attrs.h"
#include "mux/server/platform.h"
#include "template_api.h"

void mech_template_clear(Mech *mech, bool clear_communications) {
  mech_template_state_reset(mech);

  mech_spotter_dbref_set(mech, -1);
  mech_targeting_target_clear(mech);
  mech_charge_reset(mech);
  mech->rd.swarming = -1;
  mech->rd.swarmedby = -1;
  mech_dfa_target_dbref_set(mech, -1);
  mech->rd.status = MECH_STATUS_NONE;
  mech_pilot_dbref_set(mech, -1);
  mech_targeting_aim_reset(mech);
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  if (clear_communications)
    mech_communications_clear(mech);
}

bool mech_template_load(DbRef player, Mech *mech, const char *id) {
  bool clear_communications = strcmp(mech->ud.mech_type, id) != 0;
  char *filename = mech_template_resolve_path(
      mech_context(mech), btech_context_mech_template_path(mech_context(mech)),
      id);
  Mech staged;

  if (filename == nullptr)
    return false;

  /* Template loading only mutates embedded state; it must not publish staged.
   */
  staged = *mech;
  mech_template_clear(&staged, clear_communications);
  if (load_template(player, &staged, filename) < 0)
    return false;

  *mech = staged;
  silly_atr_set_in(btech_context_database(mech_context(mech)), mech->mynum,
                   A_MECHTYPE, mech->ud.mech_type);
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  return true;
}
