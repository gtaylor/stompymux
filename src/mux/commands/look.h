/* look.h - Object look and inventory display helper interface. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

constexpr int LOOK_LOOK = 1;      /* List description. */
constexpr int LOOK_INVENTORY = 4; /* List inventory. */
constexpr int LOOK_OUTSIDE = 8;   /* Look outside a container. */

constexpr int LK_SHOWATTR = 0x0004;
constexpr int LK_SHOWEXIT = 0x0008;

void look_in(EvaluationContext *evaluation, DbRef player, DbRef location,
             int key);
