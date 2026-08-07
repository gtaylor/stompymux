#include "repair_job.h"

intptr_t repair_event_payload_pack(RepairEventPayload payload) {
  return payload.location + payload.position * LOCMAX +
         payload.extra * LOCMAX * POSMAX + payload.player * PLAYERPOS;
}

RepairEventPayload repair_event_payload_unpack(intptr_t encoded) {
  intptr_t value = encoded % PLAYERPOS;
  return (RepairEventPayload){
      .location = value % LOCMAX,
      .position = (value / LOCMAX) % POSMAX,
      .extra = value / (LOCMAX * POSMAX),
      .player = encoded / PLAYERPOS,
  };
}
