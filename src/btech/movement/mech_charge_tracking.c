#include "mech_charge_tracking_api.h"

#include <math.h>

#include "btconfig.h"
#include "btech/context.h"
#include "equipment_types.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_targeting_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"

void mech_charge_timeout_update(Mech *mech) {
  if (!btech_context_uses_new_charge_rules(mech_context(mech)) ||
      mech_charge_target_dbref(mech) <= 0)
    return;
  if (mech_charge_timer_advance(mech) <= CHARGE_TIMER_LIMIT)
    return;

  mech_notify(mech, MECHALL, "Charge timed out, charge reset.");
  mech_charge_reset(mech);
}

void mech_charge_distance_record(Mech *mech, float delta_x, float delta_y) {
  if (!btech_context_uses_new_charge_rules(mech_context(mech)) ||
      mech_charge_target_dbref(mech) <= 0)
    return;

  float x_scale = 1.0F / (float)SCALEMAP;
  float distance = sqrtf(x_scale * x_scale * delta_x * delta_x +
                         (float)YSCALE2 * delta_y * delta_y);
  mech_charge_distance_add(mech, distance);
}

void mech_charge_impact_resolve(Mech *mech) {
  DbRef target_dbref = mech_charge_target_dbref(mech);
  if (target_dbref == -1)
    return;

  Mech *target = btech_context_get_mech(mech_context(mech), target_dbref);
  if (!target) {
    mech_notify(mech, MECHPILOT, "Invalid CHARGE target!");
    mech_charge_reset(mech);
    return;
  }
  if (mech_range_to(mech, target) >= (float)CHARGE_DIST_TRIGGER)
    return;

  charge_mech(mech, target);
  mech_charge_reset(mech);
}
