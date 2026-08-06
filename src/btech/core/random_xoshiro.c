#include "random.h"

#include <assert.h>
#include <errno.h>
#include <stddef.h>
#include <sys/random.h>

static uint64_t rotate_left(uint64_t value, int count) {
  return (value << count) | (value >> (64 - count));
}

static uint64_t splitmix64_next(uint64_t *state) {
  uint64_t value = (*state += UINT64_C(0x9e3779b97f4a7c15));

  value = (value ^ (value >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
  value = (value ^ (value >> 27)) * UINT64_C(0x94d049bb133111eb);
  return value ^ (value >> 31);
}

void btech_random_seed(BtechRandom *random, uint64_t seed) {
  assert(random != nullptr);

  for (size_t index = 0; index < BTECH_RANDOM_STATE_SIZE; index++) {
    random->state[index] = splitmix64_next(&seed);
  }
  random->initialized = true;
}

bool btech_random_seed_from_system(BtechRandom *random) {
  uint64_t seed;
  size_t offset = 0;

  assert(random != nullptr);

  while (offset < sizeof(seed)) {
    ssize_t bytes = getrandom((char *)&seed + offset, sizeof(seed) - offset, 0);

    if (bytes > 0) {
      offset += (size_t)bytes;
      continue;
    }
    if (bytes < 0 && errno == EINTR) {
      continue;
    }
    return false;
  }

  btech_random_seed(random, seed);
  return true;
}

uint64_t btech_random_u64(BtechRandom *random) {
  uint64_t result;
  uint64_t temporary;

  assert(random != nullptr);
  assert(random->initialized);

  result = rotate_left(random->state[1] * UINT64_C(5), 7) * UINT64_C(9);
  temporary = random->state[1] << 17;

  random->state[2] ^= random->state[0];
  random->state[3] ^= random->state[1];
  random->state[1] ^= random->state[2];
  random->state[0] ^= random->state[3];
  random->state[2] ^= temporary;
  random->state[3] = rotate_left(random->state[3], 45);

  return result;
}

long btech_random_i31(BtechRandom *random) {
  return (long)(btech_random_u64(random) >> 33);
}
