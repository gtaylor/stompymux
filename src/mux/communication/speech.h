/* speech.h - Player speech and private-message command declarations. */

#pragma once

#include "mux/database/db.h"

typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

void do_pemit_list(EvaluationContext *evaluation, DbRef player, char *list,
                   const char *message);
