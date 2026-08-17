/* Implements structural validation for repair event payloads. */

#include "repair_event_validation.h"

#include "equipment_types.h"
#include "mech_events.h"
#include "repair_job.h"

static bool repair_event_section_valid(int section) {
  return (section >= 0 && section < NUM_SECTIONS) != 0;
}

static bool repair_event_critical_coordinate_valid(int section, int position) {
  return (repair_event_section_valid(section) && position >= 0 &&
          position < POSMAX) != 0;
}

bool repair_event_payload_structurally_valid(int event_type,
                                             RepairEventPayload payload,
                                             bool fake) {
  int amount = repair_fix_event_amount(payload);
  bool fix_encoding_valid;
  if (fake)
    fix_encoding_valid = (payload.position == 0 && payload.extra == 0) != 0;
  else
    fix_encoding_valid =
        (payload.position >= 0 && payload.position < POSMAX &&
         payload.extra >= 0 && payload.extra < POSMAX && amount > 0) != 0;

  switch (event_type) {
  case EVENT_REPAIR_FIX:
    return (payload.location >= 0 && payload.location < (NUM_SECTIONS * 2) &&
            fix_encoding_valid) != 0;
  case EVENT_REPAIR_FIXI:
    return (repair_event_section_valid(payload.location) &&
            fix_encoding_valid) != 0;
  case EVENT_REPAIR_REAT:
  case EVENT_REPAIR_RESE:
  case EVENT_REPAIR_REPSUIT:
    return repair_event_section_valid(payload.location) != 0;
  case EVENT_REPAIR_SCRL:
    return (repair_event_section_valid(payload.location) &&
            (payload.extra == 2 || payload.extra == 3)) != 0;
  case EVENT_REPAIR_REPL:
  case EVENT_REPAIR_REPAP:
  case EVENT_REPAIR_REPLG:
  case EVENT_REPAIR_REPAG:
  case EVENT_REPAIR_REPENHCRIT:
    return repair_event_critical_coordinate_valid(payload.location,
                                                  payload.position);
  case EVENT_REPAIR_RELO:
    if (!repair_event_critical_coordinate_valid(payload.location,
                                                payload.position))
      return false;
    if (fake)
      return payload.extra == 0;
    return (payload.extra >= 0 && payload.extra <= 2) != 0;
  case EVENT_REPAIR_SCRP:
  case EVENT_REPAIR_SCRG:
    return (repair_event_critical_coordinate_valid(payload.location,
                                                   payload.position) &&
            (payload.extra == 2 || payload.extra == 3)) != 0;
  case EVENT_REPAIR_MOB:
  case EVENT_REPAIR_UMOB:
    return true;
  default:
    return false;
  }
}
