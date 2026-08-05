#include "mech_update_api.h"

int mech_skid_modifier(float speed) {
  if (speed < 2.1F)
    return -1;
  if (speed < 4.1F)
    return 0;
  if (speed < 7.1F)
    return 1;
  if (speed < 10.1F)
    return 2;
  return 4;
}
