#include "mech_electronics_api.h"

#include "mech_internal.h"

int mech_computer_quality(const Mech *mech) { return mech->ud.computer; }

int mech_radio_quality(const Mech *mech) { return mech->ud.radio; }

int mech_radio_range(const Mech *mech) { return mech->ud.radio_range; }

void mech_radio_range_set(Mech *mech, int range) {
  mech->ud.radio_range = range;
}

void mech_radio_range_add(Mech *mech, int amount) {
  mech->ud.radio_range += amount;
}

int mech_tactical_range(const Mech *mech) { return mech->ud.tac_range; }

void mech_tactical_range_set(Mech *mech, int range) {
  mech->ud.tac_range = range;
}

int mech_long_range_sensor_range(const Mech *mech) {
  return mech->ud.lrs_range;
}

void mech_long_range_sensor_range_set(Mech *mech, int range) {
  mech->ud.lrs_range = range;
}

int mech_scanner_range(const Mech *mech) { return mech->ud.scan_range; }

void mech_scanner_range_set(Mech *mech, int range) {
  mech->ud.scan_range = range;
}
