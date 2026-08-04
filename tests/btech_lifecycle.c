#include "mech.lifecycle.h"

#include "btmacros.h"
#include "mech.h"

static int speed_corrections;
static int los_updates;

void correct_speed(MECH *mech) {
  (void)mech;
  speed_corrections++;
}

void MarkForLOSUpdate(MECH *mech) {
  (void)mech;
  los_updates++;
}

int main(void) {
  MECH mech = {0};

  MechCritStatus(&mech) = SPEED_OK;
  mech_max_speed_set(&mech, 12.0F);
  mech_max_speed_lower(&mech, 3.0F);
  mech_max_speed_divide(&mech, 3.0F);
  if (MechMaxSpeed(&mech) != 3.0F ||
      (MechCritStatus(&mech) & SPEED_OK) || speed_corrections != 3) {
    return 1;
  }

  MechStatus(&mech) = FALLEN;
  mech_make_stand(&mech);
  return !Fallen(&mech) && los_updates == 1 ? 0 : 1;
}
