/* builder_compile_object_name.c -- compiled object-name ownership tests */

#include <stdarg.h>
#include <string.h>

#include "mux/commands/builder_commands_internal.h"
#include "mux/commands/command_context.h"
#include "mux/network/network_output.h"
#include "mux/support/owned_text.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"

static bool notified;

bool styled_text_compile(const StyledTextPalette *palette [[maybe_unused]],
                         const char *markup, char *output [[maybe_unused]],
                         size_t output_size [[maybe_unused]], char *error,
                         size_t error_size) {
  if (strcmp(markup, "invalid") != 0)
    return true;
  (void)string_copy_bounded(error, error_size, "bad markup");
  return false;
}

void notify_printf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {
  notified = true;
}

int main(void) {
  WorldContext world = {};
  EvaluationContext evaluation = {.world = &world};

  OwnedText valid = builder_compile_object_name(&evaluation, 1, "valid");
  if (valid.owned == nullptr || strcmp(valid.text, "valid") != 0)
    return 1;
  owned_text_release(&valid);
  if (notified)
    return 3;

  OwnedText invalid = builder_compile_object_name(&evaluation, 1, "invalid");
  if (invalid.text != nullptr || invalid.owned != nullptr || !notified)
    return 2;
  owned_text_release(&invalid);
  return 0;
}
