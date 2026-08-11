#include "repair_job.h"

#include <stdint.h>

int main(void) {
  RepairEventPayload payload = {
      .location = 3,
      .position = 4,
      .extra = 5,
      .player = 42,
  };
  intptr_t encoded = repair_event_payload_pack(payload);
  if (encoded != 2753859)
    return 1;

  RepairEventPayload decoded = repair_event_payload_unpack(encoded);
  if (decoded.location != payload.location ||
      decoded.position != payload.position || decoded.extra != payload.extra ||
      decoded.player != payload.player)
    return 1;

  decoded = repair_event_payload_unpack(
      repair_event_payload_pack((RepairEventPayload){
          .location = LOCMAX - 1,
          .position = POSMAX - 1,
          .extra = EXTMAX - 1,
          .player = 1073741824,
      }));
  if (decoded.location != LOCMAX - 1 || decoded.position != POSMAX - 1 ||
      decoded.extra != EXTMAX - 1 || decoded.player != 1073741824)
    return 1;

  decoded = repair_event_payload_unpack(INTPTR_MAX);
  if (decoded.location < 0 || decoded.location >= LOCMAX ||
      decoded.position < 0 || decoded.position >= POSMAX || decoded.extra < 0)
    return 1;
  return 0;
}
