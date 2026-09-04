/* Implements the repair-operation message catalogue. */

#include "repair_need_messages.h"

#include <stdlib.h>

#include "mux/support/checked_storage.h"

static const char *const REPAIR_NEED_MESSAGES[] = {
    "Reattachment",
    "Repairs on %s",
    "Repairs on %s",
    "Repairs on %s",
    "Realign focus on %s",
    "Charging crystal repairs on %s",
    "Barrel repairs on %s",
    "Ammo feed repairs on %s",
    "Ranging system repairs on %s",
    "Ammo feed repairs on %s",
    "Replacement of %s",
    "Reload of %s%s (%d rounds)",
    "Repairs on%s armor (%d points)",
    "Repairs on rear%s armor (%d points)",
    "Repairs on%s internals (%d points)",
    "Removal of section",
    "Removal of %s",
    "Removal of %s",
    "Unload of %s%s(%d rounds)",
    "Reseal",
    "Replace suit",
};

const char *repair_need_message(int type) {
  if (type < 0)
    abort();
  const char *const *message = (const char *const *)checked_storage_at_const(
      (const void *)REPAIR_NEED_MESSAGES,
      sizeof(REPAIR_NEED_MESSAGES) / sizeof(*REPAIR_NEED_MESSAGES),
      sizeof(*REPAIR_NEED_MESSAGES), (size_t)type);
  return *message;
}
