/** @file
 * MUX-to-BTech command dispatch boundary.
 */

#pragma once

#include <stdbool.h>

#include "btech/ids.h"

typedef struct BtechContext BtechContext;

/**
 * Attempts to dispatch a command to the BTech command subsystem.
 *
 * @param[in,out] context BTech runtime context.
 * @param[in] player Object issuing the command.
 * @param[in] location Object containing the player.
 * @param[in,out] command Mutable command text consumed during parsing.
 * @return `true` when BTech handled the command; otherwise `false`.
 */
bool btech_command_try_execute(BtechContext *context, BtechObjectId player,
                               BtechObjectId location, char *command);
