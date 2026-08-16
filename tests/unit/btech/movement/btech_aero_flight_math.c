#include "aero_move_api.h"

#include <assert.h>
#include <float.h>
#include <math.h>

static void expect_close(double expected, double actual) {
  const double SCALE = fmax(1.0, fabs(expected));
  assert(fabs(expected - actual) <= 1.0e-12 * SCALE);
}

static void test_hypotenuse(void) {
  expect_close(5.0, length_hypotenuse(-3.0, -4.0));

  /* Squaring first loses a finite result to overflow. */
  assert(isfinite(length_hypotenuse(DBL_MAX, 1.0)));
  expect_close(DBL_MAX, length_hypotenuse(DBL_MAX, 1.0));

  /* hypot also preserves subnormal-scale results that direct squaring loses. */
  assert(length_hypotenuse(1.0e-300, 1.0e-300) > 0.0);
}

static void test_difference_root(void) {
  expect_close(sqrt(7.0), my_sqrtm(3.0, 4.0));
  expect_close(sqrt(7.0), my_sqrtm(-4.0, 3.0));
  assert(isfinite(my_sqrtm(DBL_MAX, 1.0)));
  expect_close(DBL_MAX, my_sqrtm(DBL_MAX, 1.0));

  assert(isinf(my_sqrtm((double)INFINITY, 1.0)));
  assert(isnan(my_sqrtm((double)NAN, 1.0)));
  assert(isnan(my_sqrtm((double)INFINITY, (double)INFINITY)));
}

int main(void) {
  test_hypotenuse();
  test_difference_root();
  return 0;
}
