#include "mech_sensor_state_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

int mech_sensor_index(const Mech *mech, int slot) {
  return mech->rd.sensor[slot];
}

void mech_sensors_set(Mech *mech, int primary, int secondary) {
  mech->rd.sensor[0] = primary;
  mech->rd.sensor[1] = secondary;
}

bool mech_is_fallen(const Mech *mech) { return mech->rd.status & FALLEN; }

bool mech_is_jellied(const Mech *mech) { return mech->rd.critstatus & JELLIED; }

bool mech_searchlight_active(const Mech *mech) {
  return mech->rd.status2 & SLITE_ON;
}

bool mech_has_searchlight(const Mech *mech) {
  return mech->rd.specials & SLITE_TECH;
}

bool mech_has_operational_beagle_probe(const Mech *mech) {
  return (mech->rd.specials & BEAGLE_PROBE_TECH) &&
         !(mech->rd.critstatus & BEAGLE_DESTROYED);
}

bool mech_has_operational_bloodhound_probe(const Mech *mech) {
  return (mech->rd.specials2 & BLOODHOUND_PROBE_TECH) &&
         !(mech->rd.critstatus & BLOODHOUND_DESTROYED);
}

bool mech_is_clairvoyant(const Mech *mech) {
  return mech->rd.critstatus & CLAIRVOYANT;
}

bool mech_is_ecm_disturbed(const Mech *mech) {
  return mech->rd.status2 & ECM_DISTURBANCE;
}

bool mech_is_any_ecm_disturbed(const Mech *mech) {
  return mech->rd.status2 & (ECM_DISTURBANCE | ANGEL_ECM_DISTURBED);
}
