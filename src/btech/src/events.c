/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1998 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include "mech.events.h"
#include "mech.h"
#include "mux/network/mux_event.h"

#define MAX_EVENTS 100

extern char *mux_event_names[];

int mux_event_exec_count[MAX_EVENTS];

void mux_event_count_initialize() {
  int i;

  for (i = 0; i < MAX_EVENTS; i++)
    mux_event_exec_count[i] = 0;
}
[[maybe_unused]] static int mux_event_mech_event[] = {
    0, 1, 0, 1, 1, 1, 1, 1, 1, 0, /* 0-9 */
    1, 0, 1, 1, 0, 1, 1, 0, 0, 1, /*10-19 */
    1, 1, 1, 0, 0, 0, 0, 0, 0, 1, /*20-29 */
    1, 0, 1, 1, 1, 0, 1, 1, 0, 1, /*30-39 */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, /*40-49 */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, /*50-59 */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, /*60-69 */
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, /*70-79 */
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0};

void debug_EventTypes(DbRef player, void *data, char *buffer) {
  int i, j, k, tot = 0;

  if (buffer && *buffer) {
    int t[MAX_EVENTS];
    int tot_ev = 0;

    for (i = 0; i < MAX_EVENTS; i++) {
      t[i] = i;
      tot_ev += mux_event_exec_count[i];
    }
    for (i = 0; i < (MAX_EVENTS - 1); i++)
      for (j = i + 1; j < MAX_EVENTS; j++)
        if (mux_event_exec_count[t[i]] > mux_event_exec_count[t[j]]) {
          int s = t[i];

          t[i] = t[j];
          t[j] = s;
        }
    /* Then, display */
    notify(BTECH_EVALUATION_CONTEXT, player, "Event history (by use)");
    for (i = 0; i < MAX_EVENTS; i++)
      if (mux_event_exec_count[t[i]])
        notify_printf(BTECH_EVALUATION_CONTEXT, player, "%-3d%-20s%10d %.3f%%",
                      t[i], mux_event_names[t[i]], mux_event_exec_count[t[i]],
                      ((float)100.0 * mux_event_exec_count[t[i]] /
                       (tot_ev ? tot_ev : 1)));

    return;
  }
  notify(BTECH_EVALUATION_CONTEXT, player, "Events by type: ");
  notify(BTECH_EVALUATION_CONTEXT, player, "-------------------------------");
  k = mux_event_last_type(btech_context_active()->events);
  for (i = 0; i <= k; i++) {
    j = mux_event_count_type(btech_context_active()->events, i);
    if (!j)
      continue;
    tot += j;
    notify_printf(BTECH_EVALUATION_CONTEXT, player, "%-20s%d",
                  mux_event_names[i], j);
  }
  if (tot)
    notify(BTECH_EVALUATION_CONTEXT, player, "-------------------------------");
  notify_printf(BTECH_EVALUATION_CONTEXT, player, "%d total", tot);
}

void prerun_event(MuxEvent *e) {}

void postrun_event(MuxEvent *e) {}
