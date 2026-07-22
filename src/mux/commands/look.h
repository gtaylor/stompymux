/* look.h - Object look and inventory display helper interface. */

#pragma once

#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;

void look_in(EvaluationContext *evaluation, DbRef player, DbRef location,
             int key);
