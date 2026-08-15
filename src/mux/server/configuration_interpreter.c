/* Mutable-text adapter for configuration value interpreters. */

#include "mux/server/configuration_interpreter.h"

#include <stdlib.h>
#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

int configuration_interpreter_invoke_with_mutable_text(
    ConfigurationInterpreter interpreter, ConfigurationCall *call,
    const char *text) {
  const size_t TEXT_CAPACITY = strlen(text) + 1;
  char *mutable_text =
      checked_storage_allocate_array(TEXT_CAPACITY, sizeof(char));
  (void)string_copy_bounded(mutable_text, TEXT_CAPACITY, text);
  call->text = mutable_text;

  const int RESULT = interpreter(call);

  call->text = nullptr;
  free(mutable_text);
  return RESULT;
}
