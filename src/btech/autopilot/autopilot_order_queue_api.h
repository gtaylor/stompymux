/* Safe ownership and mutation API for queued autopilot orders. */

#pragma once

#include <stddef.h>

#include "autopilot.h"

typedef enum AutopilotOrderResult {
  AUTOPILOT_ORDER_OK,
  AUTOPILOT_ORDER_INVALID,
  AUTOPILOT_ORDER_UNSUPPORTED,
  AUTOPILOT_ORDER_FULL,
  AUTOPILOT_ORDER_NOT_FOUND,
  AUTOPILOT_ORDER_NO_MEMORY,
} AutopilotOrderResult;

bool autopilot_order_is_supported(int command_enum);
size_t autopilot_order_count(const Autopilot *autopilot);
const AutopilotCommand *autopilot_order_at(const Autopilot *autopilot,
                                           size_t index);
AutopilotOrderResult
autopilot_order_enqueue(Autopilot *autopilot,
                        const AutopilotCommandDefinition *definition,
                        const AutopilotArgumentList *arguments);
AutopilotOrderResult autopilot_order_remove(Autopilot *autopilot, size_t index);
void autopilot_order_clear(Autopilot *autopilot);
AutopilotOrderResult autopilot_order_pop(Autopilot *autopilot);
