#include "aero_move_api.h"

#include <math.h>

#include "mech_classification_api.h"
#include "mech_update_api.h"
#include "section_types.h"

void aero_heading_update(Mech *mech) {
  if (mech_class(mech) == CLASS_SPHEROID_DS)
    mech_heading_update(mech);
}

double length_hypotenuse(double x, double y) {
  if (x < 0)
    x = -x;
  if (y < 0)
    y = -y;
  return sqrt(x * x + y * y);
}

double my_sqrtm(double x, double y) {
  if (x < 0)
    x = -x;
  if (y < 0)
    y = -y;
  if (y > x) {
    double swap = y;
    y = x;
    x = swap;
  }
  return sqrt(x * x - y * y);
}
