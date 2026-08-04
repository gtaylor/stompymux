/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1998 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include "btech/context.h"
#include "btech_event.h"
#include "legacy_macros.h"
#include "map_obj_api.h"
#include "mux/network/mux_event.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "special_object.h"

void debug_EventTypes(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  int i, j, k, tot = 0;

  (void)buffer;
  notify(btech_context_evaluation(debug->context), player, "Events by type: ");
  notify(btech_context_evaluation(debug->context), player,
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
    notify(btech_context_evaluation(debug->context), player,
           "-------------------------------");
  notify_printf(btech_context_evaluation(debug->context), player, "%d total",
                tot);
}
