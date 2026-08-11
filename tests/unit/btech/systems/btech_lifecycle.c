#include "mech_lifecycle.h"
#include "mech_sensor_state_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"

#include "crit_api.h"

#include "mech_internal.h"

static int speed_corrections;
static int los_updates;

void mech_speed_correct(Mech *mech) {
  (void)mech;
  speed_corrections++;
}

void mark_for_los_update(Mech *mech) {
  (void)mech;
  los_updates++;
}

int main(void) {
  Mech mech = {0};

  ((&mech)->rd.critstatus) = SPEED_OK;
  mech_max_speed_set(&mech, 12.0F);
  mech_max_speed_lower(&mech, 3.0F);
  mech_max_speed_divide(&mech, 3.0F);
  if (((&mech)->ud.maxspeed) != 3.0F || (((&mech)->rd.critstatus) & SPEED_OK) ||
      speed_corrections != 3) {
    return 1;
  }

  ((&mech)->rd.status) = FALLEN;
  mech_make_stand(&mech);
  return !mech_is_fallen(&mech) && los_updates == 1 ? 0 : 1;
}
