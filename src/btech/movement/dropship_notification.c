#include "mech_api_types.h"
#include "mech_update_api.h"

#include "btech/context.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
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

void dropship_notification_broadcast(Mech *mech, const char *message) {
  if (mech_is_dropship(mech))
    mech_los_broadcast(mech, message);
}

void dropship_notification_broadcast_if_due(Mech *mech, const char *message) {
  if (mech_is_dropship(mech) && dropship_notification_is_due(mech))
    mech_los_broadcast(mech, message);
}
