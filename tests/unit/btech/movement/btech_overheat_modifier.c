#include "mech_update_api.h"

#include <assert.h>
#include <math.h>

#include "mech_heat_api.h"

static float excess_heat;

float mech_excess_heat(const Mech *mech [[maybe_unused]]) {
  return excess_heat;
}

static void expect_modifier(float heat, int expected) {
  excess_heat = heat;
  assert(mech_overheat_to_hit_modifier(nullptr) == expected);
}

static void test_thresholds(void) {
  expect_modifier(-INFINITY, 0);
  expect_modifier(0.0F, 0);
  expect_modifier(7.999F, 0);
  expect_modifier(8.0F, 1);
  expect_modifier(12.999F, 1);
  expect_modifier(13.0F, 2);
  expect_modifier(16.999F, 2);
  expect_modifier(17.0F, 3);
  expect_modifier(23.999F, 3);
  expect_modifier(24.0F, 4);
  expect_modifier(INFINITY, 4);
}

static void test_nan_is_not_a_modifier(void) {
  excess_heat = NAN;
  assert(mech_overheat_to_hit_modifier(nullptr) == 0);
}

int main(void) {
  test_thresholds();
  test_nan_is_not_a_modifier();
  return 0;
}
