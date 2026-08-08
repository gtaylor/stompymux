/* Owning, bounds-checked autopilot command arguments. */

#include "autopilot_argument_list_api.h"

#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"

static char **autopilot_argument_list_slot(AutopilotArgumentList *arguments,
                                           size_t index) {
  return checked_storage_at(arguments->values, arguments->capacity,
                            sizeof(char *), index);
}

void autopilot_argument_list_initialize(AutopilotArgumentList *arguments,
                                        size_t capacity) {
  if (capacity == 0 || capacity > AUTOPILOT_MAX_ARGS)
    abort();
  memset(arguments, 0, sizeof(*arguments));
  arguments->capacity = capacity;
}

char **
autopilot_argument_list_parser_storage(AutopilotArgumentList *arguments) {
  return arguments->values;
}

const char *autopilot_argument_list_get(AutopilotArgumentList *arguments,
                                        size_t index) {
  return *autopilot_argument_list_slot(arguments, index);
}

void autopilot_argument_list_set(AutopilotArgumentList *arguments, size_t index,
                                 char *value) {
  *autopilot_argument_list_slot(arguments, index) = value;
}

char *autopilot_argument_list_take(AutopilotArgumentList *arguments,
                                   size_t index) {
  char **slot = autopilot_argument_list_slot(arguments, index);
  char *value = *slot;
  *slot = nullptr;
  return value;
}

void autopilot_argument_list_destroy(AutopilotArgumentList *arguments) {
  for (size_t index = 0; index < arguments->capacity; index++)
    free(autopilot_argument_list_take(arguments, index));
  arguments->capacity = 0;
}
