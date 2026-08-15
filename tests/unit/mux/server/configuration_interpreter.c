/* configuration_interpreter.c -- mutable configuration text adapter tests */

#include <string.h>

#include "mux/server/configuration_interpreter.h"

static int mutate_text(const ConfigurationCall *call) {
  if (strcmp(call->text, "unchanged") != 0)
    return -1;
  call->text[0] = 'X';
  return 17;
}

int main(void) {
  const char original[] = "unchanged";
  ConfigurationCall call = {};

  if (configuration_interpreter_invoke_with_mutable_text(mutate_text, &call,
                                                         original) != 17)
    return 1;
  if (strcmp(original, "unchanged") != 0)
    return 2;
  return call.text == nullptr ? 0 : 3;
}
