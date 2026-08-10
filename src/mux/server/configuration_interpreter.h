/* Defines a typed configuration-value interpreter invocation. */

#pragma once

#include "mux/server/platform.h"

typedef struct ConfigurationContext ConfigurationContext;

typedef struct ConfigurationCall {
  void *value;
  char *text;
  long extra;
  DbRef player;
  char *command;
  ConfigurationContext *context;
} ConfigurationCall;

typedef int (*ConfigurationInterpreter)(const ConfigurationCall *call);
