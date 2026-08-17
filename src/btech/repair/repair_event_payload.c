#include "repair_job.h"

#include "checked_conversion.h"
#include <stddef.h>
#include <stdint.h>

intptr_t repair_event_payload_pack(RepairEventPayload payload) {
  return payload.location + (payload.position * LOCMAX) +
         (payload.extra * LOCMAX * POSMAX) + (payload.player * PLAYERPOS);
}

RepairEventPayload repair_event_payload_unpack(intptr_t encoded) {
  intptr_t value = encoded % PLAYERPOS;
  return (RepairEventPayload){
      .location = clamp_intptr_to_int(value % LOCMAX),
      .position = clamp_intptr_to_int((value / LOCMAX) % POSMAX),
      .extra = clamp_intptr_to_int(value / ((intptr_t)(LOCMAX * POSMAX))),
      .player = encoded / PLAYERPOS,
  };
}

int repair_fix_event_amount(RepairEventPayload payload) {
  return payload.position + ((payload.extra % POSMAX) * POSMAX);
}

bool repair_fix_event_payload_with_amount(RepairEventPayload *payload,
                                          int amount) {
  if (amount <= 0 || amount > REPAIR_FIX_AMOUNT_MAX)
    return false;
  payload->position = amount % POSMAX;
  payload->extra = amount / POSMAX;
  return true;
}
