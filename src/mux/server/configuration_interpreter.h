/* Defines a typed configuration-value interpreter invocation. */

#pragma once

#include <stdint.h>

#include "mux/server/platform.h"

typedef struct ConfigurationContext ConfigurationContext;

typedef struct ConfigurationCall {
  void *value;
  char *text;
  intptr_t extra;
  DbRef player;
  char *command;
  ConfigurationContext *context;
} ConfigurationCall;

typedef int (*ConfigurationInterpreter)(const ConfigurationCall *call);
