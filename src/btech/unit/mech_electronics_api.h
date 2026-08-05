#pragma once

#include "mech_api_types.h"

int mech_computer_quality(const Mech *mech);
int mech_radio_quality(const Mech *mech);
void mech_radio_quality_set(Mech *mech, int quality);
int mech_radio_configuration(const Mech *mech);
void mech_radio_configuration_set(Mech *mech, int configuration);
int mech_radio_range(const Mech *mech);
void mech_radio_range_set(Mech *mech, int range);
void mech_radio_range_add(Mech *mech, int amount);
int mech_tactical_range(const Mech *mech);
void mech_tactical_range_set(Mech *mech, int range);
int mech_long_range_sensor_range(const Mech *mech);
void mech_long_range_sensor_range_set(Mech *mech, int range);
int mech_scanner_range(const Mech *mech);
void mech_scanner_range_set(Mech *mech, int range);
void mech_sensor_ranges_halve(Mech *mech);
void mech_sensor_ranges_disable(Mech *mech);
