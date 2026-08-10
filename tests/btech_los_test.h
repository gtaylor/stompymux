#pragma once

#include <stdbool.h>
#include <stdio.h>

typedef struct LosTestState {
  int failures;
  int checks;
} LosTestState;

static inline void los_expect_int(LosTestState *state, const char *name,
                                  int expected, int actual) {
  ++state->checks;
  if (expected == actual)
    return;
  ++state->failures;
  (void)fprintf(stderr, "%s: expected %d, got %d\n", name, expected, actual);
}

static inline void los_expect_true(LosTestState *state, const char *name,
                                   bool actual) {
  los_expect_int(state, name, 1, actual ? 1 : 0);
}

/* Known divergences must be reviewed when behavior reaches the intended value.
 */
static inline void los_expect_divergence_int(LosTestState *state,
                                             const char *name, int legacy,
                                             int intended, int actual) {
  ++state->checks;
  if (actual == legacy)
    return;
  ++state->failures;
  if (actual == intended) {
    (void)fprintf(stderr,
                  "%s: XPASS (now returns intended value %d); promote this "
                  "case to a normal regression test\n",
                  name, intended);
    return;
  }
  (void)fprintf(stderr,
                "%s: expected legacy %d or intended %d, got unexpected %d\n",
                name, legacy, intended, actual);
}

static inline int los_test_result(const LosTestState *state) {
  if (state->failures == 0)
    return 0;
  (void)fprintf(stderr, "%d of %d LOS checks failed\n", state->failures,
                state->checks);
  return 1;
}
