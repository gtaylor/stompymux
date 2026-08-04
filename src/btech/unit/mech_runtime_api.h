#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

bool mech_is_started(const Mech *mech);
DbRef mech_autopilot_dbref(const Mech *mech);
