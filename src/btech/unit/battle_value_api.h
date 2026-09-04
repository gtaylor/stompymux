/* Declares computed BattleTech unit Battle Value operations. */

#pragma once

#include "mech_api_types.h"
#include "mux/support/alloc.h"

/** Components of a unit's computed Battle Value. */
typedef struct BattleValue {
  double total;
  double offensive;
  double defensive;
} BattleValue;

/**
 * Calculates Battle Value from a unit's current state.
 *
 * @param mech Unit to evaluate.
 * @return Offensive, defensive, and total Battle Value components.
 */
BattleValue battle_value_calculate(Mech *mech);

/**
 * Formats a unit's current total Battle Value for script-value access.
 *
 * @param mech Unit to evaluate.
 * @param buffer Caller-owned output buffer.
 * @return buffer.
 */
char *battle_value_format(Mech *mech, char buffer[static LBUF_SIZE]);
