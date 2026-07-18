/* runtime_clock.c - Process time, maintenance deadlines, and usage samples. */

#include "mux/server/runtime_clock.h"

#include <assert.h>
#include <string.h>

void runtime_clock_initialize(RuntimeClock *clock) {
  assert(clock != nullptr);
  memset(clock, 0, sizeof(*clock));
  clock->now = time(nullptr);
  clock->events_last_hour = -1;
}

