#include "mech_position_api.h"

#include "mech_internal.h"

void mech_position_reset_origin(Mech *mech) {
  mech->pd.last_x = 0;
  mech->pd.last_y = 0;
  mech->pd.x = 0;
  mech->pd.y = 0;
}
