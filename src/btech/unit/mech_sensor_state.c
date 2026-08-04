#include "mech_sensor_state_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

int mech_sensor_index(const Mech *mech, int slot) {
  return mech->rd.sensor[slot];
}

bool mech_is_fallen(const Mech *mech) { return mech->rd.status & FALLEN; }

bool mech_is_jellied(const Mech *mech) { return mech->rd.critstatus & JELLIED; }

bool mech_searchlight_active(const Mech *mech) {
  return mech->rd.status2 & SLITE_ON;
}

bool mech_is_clairvoyant(const Mech *mech) {
  return mech->rd.critstatus & CLAIRVOYANT;
}
