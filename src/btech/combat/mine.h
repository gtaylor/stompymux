
/* Declares minefield combat interfaces. */

#pragma once

#include <stdbool.h>

typedef enum MineType : int {
  MINE_STANDARD = 1,
  MINE_INFERNO = 2,
  MINE_COMMAND = 3,
  MINE_VIBRA = 4,
  /* Same as vibra, except shows no message and is not destroyed. */
  MINE_TRIGGER = 5,
  MINE_LOW = MINE_STANDARD,
  MINE_HIGH = MINE_TRIGGER,
} MineType;

static inline bool mine_type_is_vibrating(int type) {
  return (type == MINE_VIBRA || type == MINE_TRIGGER) != 0;
}
