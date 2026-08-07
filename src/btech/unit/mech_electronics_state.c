#include "mech_electronics_api.h"

#include "btconfig.h"
#include "mech_internal.h"

int mech_computer_quality(const Mech *mech) { return mech->ud.computer; }

void mech_computer_quality_set(Mech *mech, int quality) {
  mech->ud.computer = quality;
}

int mech_radio_quality(const Mech *mech) { return mech->ud.radio; }

void mech_radio_quality_set(Mech *mech, int quality) {
  mech->ud.radio = quality;
}

int mech_radio_configuration(const Mech *mech) { return mech->ud.radioinfo; }

void mech_radio_configuration_set(Mech *mech, int configuration) {
  mech->ud.radioinfo = configuration;
}

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

void mech_sensor_ranges_halve(Mech *mech) {
  mech->ud.lrs_range /= 2;
  mech->ud.tac_range /= 2;
  mech->ud.scan_range /= 2;
}

void mech_sensor_ranges_disable(Mech *mech) {
  mech->ud.lrs_range = 0;
  mech->ud.tac_range = 0;
  mech->ud.scan_range = 0;
}

static float mech_computer_range_multiplier(const Mech *mech) {
  switch (mech_computer_quality(mech)) {
  case 1:
    return 0.8F;
  case 2:
    return 1.0F;
  case 3:
    return 1.25F;
  case 4:
    return 1.5F;
  case 5:
    return 1.75F;
  default:
    return 0.0F;
  }
}

static float mech_radio_range_multiplier(const Mech *mech) {
  switch (mech_radio_quality(mech)) {
  case 1:
    return 0.8F;
  case 2:
    return 1.0F;
  case 3:
    return 1.25F;
  case 4:
    return 1.5F;
  case 5:
    return 1.75F;
  default:
    return 0.0F;
  }
}

int mech_default_scanner_range(const Mech *mech) {
  return (int)(mech_computer_range_multiplier(mech) * DEFAULT_SCANRANGE);
}

int mech_default_long_range_sensor_range(const Mech *mech) {
  return (int)(mech_computer_range_multiplier(mech) * DEFAULT_LRSRANGE);
}

int mech_default_tactical_range(const Mech *mech) {
  return (int)(mech_computer_range_multiplier(mech) * DEFAULT_TACRANGE);
}

int mech_default_radio_range(const Mech *mech) {
  return (int)(DEFAULT_RADIORANGE * mech_radio_range_multiplier(mech));
}
