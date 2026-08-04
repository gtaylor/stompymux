#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

bool mech_is_started(const Mech *mech);
DbRef mech_autopilot_dbref(const Mech *mech);
void mech_autopilot_dbref_set(Mech *mech, DbRef autopilot);
void mech_seen_count_decrement(Mech *mech);
void mech_seen_count_reset(Mech *mech);
DbRef mech_carried_dbref(const Mech *mech);
void mech_carried_dbref_set(Mech *mech, DbRef carried);
bool mech_is_towed(const Mech *mech);
void mech_towed_clear(Mech *mech);
void mech_environment_conditions_set(Mech *mech, bool special, bool temperature,
                                     bool gravity, bool vacuum);
