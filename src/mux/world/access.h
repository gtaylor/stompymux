/* access.h - Object visibility, lock, and hearing permission interfaces. */

#pragma once

#include "mux/database/db.h"

typedef struct ServerConfiguration ServerConfiguration;

typedef struct EvaluationContext EvaluationContext;

int could_doit_with_context(EvaluationContext *context, DbRef player,
                            DbRef thing, int lock_number);
int can_see(EvaluationContext *evaluation,
            const ServerConfiguration *configuration, DbRef player, DbRef thing,
            int can_see_location);
void handle_ears(EvaluationContext *evaluation, DbRef thing, int could_hear,
                 int can_hear);
