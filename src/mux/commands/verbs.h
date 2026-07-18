/* verbs.h - Attribute-driven verb execution and action messaging interface. */

#pragma once

#include "mux/database/db.h"

typedef struct EvaluationContext EvaluationContext;

void did_it(EvaluationContext *evaluation, DbRef player, DbRef thing,
            int player_attribute, const char *player_default,
            int others_attribute, const char *others_default,
            int action_attribute, char *arguments[], int argument_count);
