#include "mech_update_api.h"

#include "btech/context.h"
#include "mech_identity_api.h"
#include "mech_runtime_api.h"

enum { DROPSHIP_NOTIFICATION_INTERVAL = 10 };

bool dropship_notification_is_due(Mech *mech) {
  int tick = btech_context_event_tick(mech_context(mech));
  int last_tick = mech_last_dropship_message_tick(mech);
  if (last_tick <= tick && tick - last_tick < DROPSHIP_NOTIFICATION_INTERVAL)
    return false;

  mech_last_dropship_message_tick_set(mech, tick);
  return true;
}
