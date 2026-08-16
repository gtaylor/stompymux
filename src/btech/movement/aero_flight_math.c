#include "aero_move_api.h"

#include <math.h>

#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_update_api.h"
#include "section_types.h"

void aero_heading_update(Mech *mech) {
  if (mech_class(mech) == CLASS_SPHEROID_DS)
    mech_heading_update(mech);
}

double length_hypotenuse(double x, double y) { return hypot(x, y); }

double my_sqrtm(double x, double y) {
  x = fabs(x);
  y = fabs(y);
  if (y > x) {
    double swap = y;
    y = x;
    x = swap;
  }
  if (x == 0.0)
    return 0.0;
  if (isfinite(x) && isfinite(y) && !(y < x))
    return 0.0;
  if (y <= x / 2.0)
    return x * sqrt(1.0 - ((y / x) * (y / x)));
  return sqrt(x - y) * sqrt(x + y);
}
