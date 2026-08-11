/* Implements BattleTech core services for legacy events. */

#include "btech/context.h"
#include "btech_event.h"
#include "events_api.h"
#include "map_obj_api.h"
#include "mux/network/mux_event.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "special_object.h"

void debug_event_types(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  int i, j, k, tot = 0;

  (void)buffer;
  mecha_notify(btech_context_evaluation(debug->context), player,
               "Events by type: ");
  mecha_notify(btech_context_evaluation(debug->context), player,
               "-------------------------------");
  MuxEventScheduler *events = debug->context->events;
  k = mux_event_last_type(events);
  for (i = 0; i <= k; i++) {
    j = mux_event_count_type(events, i);
    if (!j)
      continue;
    tot += j;
    notify_printf(btech_context_evaluation(debug->context), player, "%-20s%d",
                  btech_event_name(i), j);
  }
  if (tot)
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "-------------------------------");
  notify_printf(btech_context_evaluation(debug->context), player, "%d total",
                tot);
}
