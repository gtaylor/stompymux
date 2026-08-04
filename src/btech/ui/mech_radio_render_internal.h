#pragma once

#include "mech_api_types.h"

void radio_color_code(char buffer[static 32], Mech *mech, int channel,
                      int observer, int team);
