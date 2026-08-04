#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_sensor_index(const Mech *mech, int slot);
bool mech_is_fallen(const Mech *mech);
bool mech_is_jellied(const Mech *mech);
bool mech_searchlight_active(const Mech *mech);
bool mech_is_clairvoyant(const Mech *mech);
