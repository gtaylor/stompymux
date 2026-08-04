#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_sensor_index(const Mech *mech, int slot);
void mech_sensors_set(Mech *mech, int primary, int secondary);
bool mech_is_fallen(const Mech *mech);
bool mech_is_jellied(const Mech *mech);
bool mech_searchlight_active(const Mech *mech);
bool mech_has_searchlight(const Mech *mech);
bool mech_has_operational_beagle_probe(const Mech *mech);
bool mech_has_operational_bloodhound_probe(const Mech *mech);
bool mech_is_clairvoyant(const Mech *mech);
bool mech_is_ecm_disturbed(const Mech *mech);
bool mech_is_any_ecm_disturbed(const Mech *mech);
