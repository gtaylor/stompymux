/** @file
 * Defines a typed configuration-value interpreter invocation.
 */
#pragma once

#include <stdint.h>

#include "mux/server/platform.h"

typedef struct ConfigurationContext ConfigurationContext;

typedef struct ConfigurationCall {
  void *value;
  char *text;
  intptr_t extra;
  DbRef player;
  const char *command;
  ConfigurationContext *context;
} ConfigurationCall;

typedef int (*ConfigurationInterpreter)(const ConfigurationCall *call);

/** Executes configuration interpreter invoke with mutable text. @param[in]
 * interpreter Interpreter. @param[in,out] call Call. @param[in] text Text to
 * process. */

int configuration_interpreter_invoke_with_mutable_text(
    ConfigurationInterpreter interpreter, ConfigurationCall *call,
    const char *text);
