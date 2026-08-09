#include "repair_job.h"

#include "checked_conversion.h"
#include <stdint.h>

intptr_t repair_event_payload_pack(RepairEventPayload payload) {
  return payload.location + payload.position * LOCMAX +
         payload.extra * LOCMAX * POSMAX + payload.player * PLAYERPOS;
}

RepairEventPayload repair_event_payload_unpack(intptr_t encoded) {
  intptr_t value = encoded % PLAYERPOS;
  return (RepairEventPayload){
      .location = clamp_intptr_to_int(value % LOCMAX),
      .position = clamp_intptr_to_int((value / LOCMAX) % POSMAX),
      .extra = clamp_intptr_to_int(value / (LOCMAX * POSMAX)),
      .player = encoded / PLAYERPOS,
  };
}
