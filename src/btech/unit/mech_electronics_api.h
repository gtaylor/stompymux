#pragma once

#include "mech_api_types.h"

int mech_computer_quality(const Mech *mech);
int mech_radio_quality(const Mech *mech);
int mech_radio_range(const Mech *mech);
void mech_radio_range_set(Mech *mech, int range);
void mech_radio_range_add(Mech *mech, int amount);
int mech_tactical_range(const Mech *mech);
void mech_tactical_range_set(Mech *mech, int range);
int mech_long_range_sensor_range(const Mech *mech);
void mech_long_range_sensor_range_set(Mech *mech, int range);
int mech_scanner_range(const Mech *mech);
void mech_scanner_range_set(Mech *mech, int range);
