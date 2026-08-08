#include <limits.h>
#include <stdint.h>
#include <string.h>

#include "btech/context.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "random.h"

typedef struct TestRange {
  long low;
  long high;
} TestRange;

static const uint64_t *expected_value(const uint64_t *values, size_t count,
                                      size_t index) {
  return checked_storage_at_const(values, count, sizeof(*values), index);
}

static const TestRange *test_range(const TestRange *ranges, size_t count,
                                   size_t index) {
  return checked_storage_at_const(ranges, count, sizeof(*ranges), index);
}

static const uint64_t *random_state_value(const BtechRandom *random,
                                          size_t index) {
  return checked_storage_at_const(random->state, BTECH_RANDOM_STATE_SIZE,
                                  sizeof(*random->state), index);
}

static int test_reference_sequence(void) {
  static const uint64_t expected[] = {
      UINT64_C(0xb3f2af6d0fc710c5), UINT64_C(0x853b559647364cea),
      UINT64_C(0x92f89756082a4514), UINT64_C(0x642e1c7bc266a3a7),
      UINT64_C(0xb27a48e29a233673),
  };
  BtechRandom random = {0};

  btech_random_seed(&random, UINT64_C(1));
  for (size_t index = 0; index < sizeof(expected) / sizeof(expected[0]);
       index++) {
    if (btech_random_u64(&random) !=
        *expected_value(expected, sizeof(expected) / sizeof(expected[0]),
                        index)) {
      return 1;
    }
  }
  return 0;
}

static int test_seed_behavior(void) {
  BtechRandom first = {0};
  BtechRandom second = {0};
  BtechRandom third = {0};
  BtechRandom zero_seed = {0};

  btech_random_seed(&first, UINT64_C(42));
  btech_random_seed(&second, UINT64_C(42));
  btech_random_seed(&third, UINT64_C(43));
  btech_random_seed(&zero_seed, 0);

  if (memcmp(first.state, second.state, sizeof(first.state)) != 0 ||
      memcmp(first.state, third.state, sizeof(first.state)) == 0) {
    return 1;
  }
  for (int draw = 0; draw < 10; draw++) {
    if (btech_random_u64(&first) != btech_random_u64(&second)) {
      return 1;
    }
  }
  for (size_t index = 0; index < BTECH_RANDOM_STATE_SIZE; index++) {
    if (*random_state_value(&zero_seed, index) != 0) {
      return 0;
    }
  }
  return 1;
}

static int test_ranges(void) {
  static const TestRange ranges[] = {
      {0, 0},
      {-10, 10},
      {1, 6},
      {LONG_MIN, LONG_MAX},
      {LONG_MAX - 10, LONG_MAX},
  };
  BtechContext context = {0};

  btech_random_seed(&context.random, UINT64_C(99));
  for (size_t range = 0; range < sizeof(ranges) / sizeof(ranges[0]); range++) {
    const TestRange *bounds =
        test_range(ranges, sizeof(ranges) / sizeof(ranges[0]), range);
    for (int draw = 0; draw < 1000; draw++) {
      long value = btech_random_range(&context, bounds->low, bounds->high);

      if (value < bounds->low || value > bounds->high) {
        return 1;
      }
    }
  }
  return 0;
}

int main(void) {
  return test_reference_sequence() || test_seed_behavior() || test_ranges();
}
