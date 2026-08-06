#pragma once

#include <stdbool.h>
#include <stdint.h>

typedef struct BtechRollStatistics {
  int rolls[11];
  int hit_rolls[11];
  int critical_rolls[11];
  int total_rolls;
  int total_hit_rolls;
  int total_critical_rolls;
} BtechRollStatistics;

enum { BTECH_RANDOM_STATE_SIZE = 4 };

typedef struct BtechRandom {
  uint64_t state[BTECH_RANDOM_STATE_SIZE];
  BtechRollStatistics statistics;
  bool initialized;
} BtechRandom;

void btech_random_seed(BtechRandom *random, uint64_t seed);
bool btech_random_seed_from_system(BtechRandom *random);
uint64_t btech_random_u64(BtechRandom *random);
long btech_random_i31(BtechRandom *random);
