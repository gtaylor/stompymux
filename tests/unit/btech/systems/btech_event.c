#include "btech_event.h"

#include <stdbool.h>
#include <string.h>

#include <uv.h>

static int callback_count;
static bool pointer_payload_seen;

static void event_callback(MuxEvent *event [[maybe_unused]]) {
  callback_count++;
}

static void pointer_event_callback(MuxEvent *event) {
  pointer_payload_seen = event->secondary.kind == MUX_EVENT_PAYLOAD_POINTER &&
                         event->secondary.pointer != nullptr;
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

  btech_event_schedule_pointer(&scheduler, &object, 11, pointer_event_callback,
                               1, &object);
  if (mux_event_count_type_data_integer(&scheduler, 11, &object, 0) != 0 ||
      mux_event_count_type_secondary_integer(&scheduler, 11, 0) != 0)
    return 1;
  mux_event_remove_type_data_integer(&scheduler, 11, &object, 0);
  if (mux_event_run_by_type(&scheduler, 11) != 1 || !pointer_payload_seen)
    return 1;

  pointer_payload_seen = false;
  btech_event_schedule_pointer(&scheduler, &object, 12, pointer_event_callback,
                               1, &object);
  mux_event_remove_type_secondary_pointer(&scheduler, 12, &object);
  if (mux_event_run_by_type(&scheduler, 12) != 0)
    return 1;

  char *owned_payload = strdup("owned");
  if (owned_payload == nullptr)
    return 1;
  btech_event_schedule_owned_pointer(&scheduler, &object, 13,
                                     pointer_event_callback, 1, owned_payload);
  btech_event_cancel(&scheduler, &object, 13);

  owned_payload = strdup("rejected");
  if (owned_payload == nullptr)
    return 1;
  btech_event_schedule_owned_pointer(&scheduler, &object, -1,
                                     pointer_event_callback, 1, owned_payload);

  mux_event_add(&(MuxEventRequest){.scheduler = &scheduler,
                                   .delay = 1,
                                   .type = 14,
                                   .callback = event_callback,
                                   .data = &object});
  if (mux_event_count_type_data_integer(&scheduler, 14, &object, 0) != 0)
    return 1;

  /* Destroy must cancel an active timer before freeing its callback data. */
  btech_event_schedule(&scheduler, &object, 10, event_callback, 1, 0);

  mux_event_scheduler_destroy(&scheduler);
  uv_run(&loop, UV_RUN_DEFAULT);
  if (callback_count != 0 || uv_loop_close(&loop) < 0) {
    return 1;
  }
  return 0;
}
