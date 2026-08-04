/* Typed object-state command interface. */

#pragma once

#include "mux/objects/db.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

void do_state(CommandInvocation *invocation);
void state_examine_namespaces(EvaluationContext *evaluation, DbRef player,
                              DbRef object);
