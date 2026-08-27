#include "mech_tech_api.h"

#include <limits.h>
#include <stdint.h>

#include "btech/context.h"
#include "btech_event.h"
#include "mech_events.h"
#include "mux/network/mux_event.h"
#include "mux/support/checked_storage.h"
#include "repair_job.h"

static MuxEvent events[8];
static size_t event_count;
static MuxEventScheduler scheduler = {.tick = 400};
static bool variable_time;
static int time_modifier;

static void successful_callback(MuxEvent *event [[maybe_unused]]) {}
void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}

bool btech_context_uses_variable_technology_time(const BtechContext *context
                                                 [[maybe_unused]]) {
  return variable_time;
}
int btech_context_technology_time_modifier(const BtechContext *context
                                           [[maybe_unused]]) {
  return time_modifier;
}

static MuxEvent *event_at(size_t index) {
  return checked_storage_at(events, sizeof(events) / sizeof(*events),
                            sizeof(*events), index);
}

static void reset_events(void) { event_count = 0; }

static void add_event(MechEventType type, int tick, RepairEventPayload payload,
                      MuxEventCallback callback) {
  *event_at(event_count++) = (MuxEvent){
      .type = (char)type,
      .tick = tick,
      .scheduler = &scheduler,
      .function = callback,
      .secondary = {.kind = MUX_EVENT_PAYLOAD_INTEGER,
                    .integer = repair_event_payload_pack(payload)},
  };
}

void mech_event_visit(Mech *mech [[maybe_unused]], MechEventType type,
                      MuxEventVisitor visitor, void *context) {
  for (size_t index = 0; index < event_count; index++) {
    MuxEvent *event = event_at(index);
    if (event->type == type)
      visitor(event, context);
  }
}

static bool test_fix_event_amounts_extend_the_horizon(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x1;
  constexpr int OFFSET = 600;

  reset_events();
  add_event(EVENT_REPAIR_FIX, scheduler.tick + OFFSET,
            (RepairEventPayload){.location = 2, .position = 1, .player = 17},
            successful_callback);
  if (figure_latest_tech_event(mech) != OFFSET)
    return false;

  reset_events();
  RepairEventPayload armor = {.location = 2, .player = 5000000000L};
  if (!repair_fix_event_payload_with_amount(&armor, 16))
    return false;
  add_event(EVENT_REPAIR_FIX, scheduler.tick + OFFSET, armor,
            successful_callback);
  if (figure_latest_tech_event(mech) !=
      OFFSET + (15 * FIXARMOR_TIME * TECH_TICK))
    return false;

  reset_events();
  RepairEventPayload internal = {.location = 2, .player = 5000000000L};
  if (!repair_fix_event_payload_with_amount(&internal, 17))
    return false;
  add_event(EVENT_REPAIR_FIXI, scheduler.tick + OFFSET, internal,
            successful_callback);
  return figure_latest_tech_event(mech) ==
         OFFSET + (16 * FIXINTERNAL_TIME * TECH_TICK);
}

static bool test_failure_markers_keep_their_raw_offset(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x2;
  constexpr int OFFSET = 600;

  reset_events();
  add_event(EVENT_REPAIR_FIX, scheduler.tick + OFFSET,
            (RepairEventPayload){.location = 2, .player = 17},
            mech_event_failure_marker);
  if (figure_latest_tech_event(mech) != OFFSET)
    return false;

  reset_events();
  add_event(EVENT_REPAIR_FIXI, scheduler.tick + OFFSET,
            (RepairEventPayload){.location = 2, .player = 17},
            mech_event_failure_marker);
  if (figure_latest_tech_event(mech) != OFFSET)
    return false;

  reset_events();
  RepairEventPayload malformed_failure = {.location = 2, .player = 17};
  if (!repair_fix_event_payload_with_amount(&malformed_failure, 16))
    return false;
  add_event(EVENT_REPAIR_FIX, scheduler.tick + OFFSET, malformed_failure,
            mech_event_failure_marker);
  return figure_latest_tech_event(mech) == OFFSET;
}

static bool test_non_fix_events_and_maximum_amount(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x3;
  constexpr int OFFSET = 600;

  reset_events();
  add_event(EVENT_REPAIR_REPL, scheduler.tick + OFFSET,
            (RepairEventPayload){.location = 2, .position = 3, .player = 17},
            successful_callback);
  if (figure_latest_tech_event(mech) != OFFSET)
    return false;

  reset_events();
  RepairEventPayload maximum = {.location = 2, .player = 5000000000L};
  if (!repair_fix_event_payload_with_amount(&maximum, REPAIR_FIX_AMOUNT_MAX))
    return false;
  add_event(EVENT_REPAIR_FIX, scheduler.tick + OFFSET, maximum,
            successful_callback);
  return figure_latest_tech_event(mech) ==
         OFFSET + ((REPAIR_FIX_AMOUNT_MAX - 1) * FIXARMOR_TIME * TECH_TICK);
}

static bool test_adjusted_time_is_bounded_and_exact(void) {
  BtechContext *const context = (BtechContext *)(uintptr_t)0x4;
  variable_time = true;
  time_modifier = 6;
  if (tech_adjusted_time_for_roll(context, 240, 1) != 225 ||
      tech_adjusted_time_for_roll(context, 240, 2) != 211)
    return false;
  variable_time = false;
  if (tech_adjusted_time_for_roll(context, 240, 2) != 240)
    return false;
  variable_time = true;
  if (tech_adjusted_time_for_roll(context, 240, 0) != 240 ||
      tech_adjusted_time_for_roll(context, 240, -1) != 240 ||
      tech_adjusted_time_for_roll(context, 0, 2) != 0)
    return false;
  time_modifier = INT_MAX;
  if (tech_adjusted_time_for_roll(context, INT_MAX, INT_MAX) != 1)
    return false;
  time_modifier = INT_MIN;
  return tech_adjusted_time_for_roll(context, INT_MAX, INT_MAX) == INT_MAX;
}

int main(void) {
  return test_fix_event_amounts_extend_the_horizon() &&
                 test_failure_markers_keep_their_raw_offset() &&
                 test_non_fix_events_and_maximum_amount() &&
                 test_adjusted_time_is_bounded_and_exact()
             ? 0
             : 1;
}
