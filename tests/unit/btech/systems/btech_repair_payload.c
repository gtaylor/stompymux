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

  for (int amount = 1; amount <= 17; amount++) {
    RepairEventPayload amount_payload = {.location = 2, .player = 42};
    if (!repair_fix_event_payload_with_amount(&amount_payload, amount))
      return 1;
    if (repair_fix_event_amount(
            repair_event_payload_unpack(repair_event_payload_pack(amount_payload))) !=
        amount)
      return 1;
  }

  RepairEventPayload zero_amount = {.location = 2, .player = 42};
  if (repair_fix_event_payload_with_amount(&zero_amount, 0))
    return 1;

  RepairEventPayload maximum_amount = {.location = 2, .player = 42};
  if (!repair_fix_event_payload_with_amount(&maximum_amount,
                                            REPAIR_FIX_AMOUNT_MAX) ||
      repair_fix_event_amount(repair_event_payload_unpack(
          repair_event_payload_pack(maximum_amount))) !=
          REPAIR_FIX_AMOUNT_MAX ||
      repair_fix_event_payload_with_amount(&maximum_amount,
                                           REPAIR_FIX_AMOUNT_MAX + 1))
    return 1;

  decoded = repair_event_payload_unpack(INTPTR_MAX);
  if (decoded.location < 0 || decoded.location >= LOCMAX ||
      decoded.position < 0 || decoded.position >= POSMAX || decoded.extra < 0)
    return 1;
  return 0;
}
