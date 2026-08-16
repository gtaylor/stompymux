/* Declares the shared result for BattleTech text-producing helpers. */

#pragma once

#include <stdbool.h>

typedef struct BtechTextResult {
  const char *text;
  bool success;
} BtechTextResult;
