#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "btech/core/context_internal.h"
#include "btech/economy/econ_cmds_api.h"
#include "btech/special/registry_api.h"
#include "mux/server/server_config.h"

static char notification[64];

EvaluationContext *btech_context_evaluation(BtechContext *context
                                            [[maybe_unused]]) {
  return nullptr;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]], const char *message) {
  snprintf(notification, sizeof(notification), "%s", message);
}

int main(void) {
  ServerConfiguration configuration = {.btech_allow_cargo_commands = true};
  BtechContext context = {.configuration = &configuration};

  if (!mech_cargo_command_access(&context, 7) || notification[0] != '\0')
    return 1;

  configuration.btech_allow_cargo_commands = false;
  if (mech_cargo_command_access(&context, 7) ||
      strcmp(notification, "Permission denied."))
    return 1;
  return 0;
}
