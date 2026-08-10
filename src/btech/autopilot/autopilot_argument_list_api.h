/* Owning, bounds-checked autopilot command arguments. */

#pragma once

#include <stddef.h>

#include "autopilot.h"

void autopilot_argument_list_initialize(AutopilotArgumentList *arguments,
                                        size_t capacity);
char **autopilot_argument_list_parser_storage(AutopilotArgumentList *arguments);
const char *autopilot_argument_list_get(const AutopilotArgumentList *arguments,
                                        size_t index);
void autopilot_argument_list_set(AutopilotArgumentList *arguments, size_t index,
                                 char *value);
char *autopilot_argument_list_take(AutopilotArgumentList *arguments,
                                   size_t index);
void autopilot_argument_list_destroy(AutopilotArgumentList *arguments);
