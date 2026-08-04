/* Private helpers shared by builder command implementations. */

#pragma once

#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;

char *builder_compile_object_name(EvaluationContext *evaluation, DbRef player,
                                  const char *name);
