#include "btech_event.h"

#include <string.h>

#include <uv.h>

static int callback_count;

static void event_callback(MuxEvent *event [[maybe_unused]]) {
  callback_count++;
}

int main(void) {
  uv_loop_t loop;
  MuxEventScheduler scheduler;
  int object;

  if (uv_loop_init(&loop) < 0) {
    return 1;
  }
  mux_event_scheduler_initialize(&scheduler);
  mux_event_scheduler_set_loop(&scheduler, &loop);

  btech_event_schedule(&scheduler, &object, 7, event_callback, 1, 42);
  btech_event_schedule(&scheduler, &object, 7, event_callback, 1, 13);
  if (btech_event_count(&scheduler, &object, 7) != 2 ||
      btech_event_count_data(&scheduler, &object, 7, 42) != 1 ||
      btech_event_first_delay(&scheduler, &object, 7) != 13) {
    return 1;
  }

  btech_event_cancel_data(&scheduler, &object, 7, 42);
  if (btech_event_count(&scheduler, &object, 7) != 1 ||
      btech_event_data(&scheduler, &object, 7) != 13) {
    return 1;
  }
  btech_event_cancel(&scheduler, &object, 7);
  if (btech_event_count(&scheduler, &object, 7) != 0) {
    return 1;
  }

  btech_event_schedule(&scheduler, &object, 8, event_callback, 1, 0);
  btech_event_schedule(&scheduler, &object, 9, event_callback, 1, 0);
  btech_events_cancel_all(&scheduler, &object);
  if (btech_event_count(&scheduler, &object, 8) != 0 ||
      btech_event_count(&scheduler, &object, 9) != 0 ||
      strcmp(btech_event_name(1), "Move") ||
      strcmp(btech_event_name(80), "Sideslip") ||
      strcmp(btech_event_name(-1), "NONAME") ||
      strcmp(btech_event_name(81), "NONAME")) {
    return 1;
  }

  /* Destroy must cancel an active timer before freeing its callback data. */
  btech_event_schedule(&scheduler, &object, 10, event_callback, 1, 0);

  mux_event_scheduler_destroy(&scheduler);
  uv_run(&loop, UV_RUN_DEFAULT);
  if (callback_count != 0 || uv_loop_close(&loop) < 0) {
    return 1;
  }
  return 0;
}
