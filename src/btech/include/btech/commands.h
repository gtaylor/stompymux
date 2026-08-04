/* commands.h - MUX-to-BTech command dispatch boundary. */

#pragma once

#include <stdbool.h>

#include "btech/ids.h"

typedef struct BtechContext BtechContext;

bool btech_command_try_execute(BtechContext *context, BtechObjectId player,
                               BtechObjectId location, char *command);
