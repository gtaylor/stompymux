#include "mech_towing_sync_api.h"

#include "btech/context.h"
#include "mech_identity_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "registry_api.h"

void mech_towing_position_update(Mech *mech) {
  DbRef carried_dbref = mech_carried_dbref(mech);
  if (carried_dbref <= 0)
    return;

  Mech *carried = btech_context_get_mech(mech_context(mech), carried_dbref);
  if (!carried || mech_map_dbref(carried) != mech_map_dbref(mech))
    return;

  mech_position_mirror(carried, mech, 0);
  mech_heading_fixed_set(carried, mech_heading_fixed(mech));
}
