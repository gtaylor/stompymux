#include "command_invokers.h"

#include <assert.h>

#include "command_registry.h"
#include "special_object.h"
#include "value_handlers_api.h"

static BtechSpecialObject *received_object;

/* This stub replaces the real handler; this test must not link libbtech. */
void list_special_values(DbRef player [[maybe_unused]],
                         BtechSpecialObject *object,
                         const char *buffer [[maybe_unused]]) {
  received_object = object;
}

int main(void) {
  BtechSpecialObject map = {.type = GTYPE_MAP};
  char arguments[] = "";
  const BtechCommandInvocation INVOCATION = {
      .object = &map,
      .arguments = arguments,
  };

  btech_command_invoke_list_special_values(&INVOCATION);
  assert(received_object == &map);
  return 0;
}
