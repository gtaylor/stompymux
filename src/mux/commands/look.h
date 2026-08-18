/** @file
 * Object look and inventory display helper interface.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"

typedef struct EvaluationContext EvaluationContext;

constexpr int LOOK_LOOK = 1;      /* List description. */
constexpr int LOOK_INVENTORY = 4; /* List inventory. */
constexpr int LOOK_OUTSIDE = 8;   /* Look outside a container. */

constexpr int LK_SHOWATTR = 0x0004;
constexpr int LK_SHOWEXIT = 0x0008;

typedef struct LookRequest {
  EvaluationContext *evaluation;
  DbRef viewer;
  DbRef location;
  int key;
} LookRequest;

/** Executes look in. @param[in] request Request. */

void look_in(const LookRequest *request);
