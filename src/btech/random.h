#pragma once

#include <stdbool.h>

typedef struct BtechRollStatistics {
  int rolls[11];
  int hit_rolls[11];
  int critical_rolls[11];
  int total_rolls;
  int total_hit_rolls;
  int total_critical_rolls;
} BtechRollStatistics;

enum { BTECH_RANDOM_STATE_SIZE = 624 };

typedef struct BtechRandom {
  unsigned long state[BTECH_RANDOM_STATE_SIZE];
  int index;
  BtechRollStatistics statistics;
  bool initialized;
} BtechRandom;

void btech_random_seed(BtechRandom *random, unsigned long seed);
unsigned long btech_random_u32(BtechRandom *random);
long btech_random_i31(BtechRandom *random);
