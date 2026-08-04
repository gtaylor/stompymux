/* Private movement helpers shared by movement command implementations. */

#pragma once

#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;

void process_dropped_dropto(EvaluationContext *evaluation, DbRef thing,
                            DbRef player);
