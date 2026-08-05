#pragma once

#include "mux/server/platform.h"
#include "special_object.h"

typedef struct BtechContext BtechContext;
typedef struct EvaluationContext EvaluationContext;

typedef struct BtechCommandInvocation {
  BtechContext *context;
  EvaluationContext *evaluation;
  DbRef actor;
  DbRef object_id;
  BtechSpecialObject *object;
  char *arguments;
} BtechCommandInvocation;

typedef void (*BtechCommandHandler)(const BtechCommandInvocation *invocation);

typedef struct BtechCommandDefinition {
  int flag;
  const char *name;
  const char *helpmsg;
  BtechCommandHandler handler;
} BtechCommandDefinition;

static inline bool
btech_command_definition_has_handler(const BtechCommandDefinition *command) {
  return command->handler != nullptr;
}
