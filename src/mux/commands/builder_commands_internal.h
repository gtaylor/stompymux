/* Private helpers shared by builder command implementations. */

#pragma once

#include "mux/objects/db.h"
#include "mux/support/owned_text.h"

typedef struct EvaluationContext EvaluationContext;

OwnedText builder_compile_object_name(EvaluationContext *evaluation,
                                      DbRef player, const char *name);
