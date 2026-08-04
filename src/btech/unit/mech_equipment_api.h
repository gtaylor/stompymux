#pragma once

#include "mech_api_types.h"

int mech_critical_part_type(const Mech *mech, int section, int critical);
int mech_section_original_armor(const Mech *mech, int section);
int mech_section_original_rear_armor(const Mech *mech, int section);
