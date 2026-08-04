#pragma once

#include "mech_api_types.h"

DbRef mech_pilot_dbref(const Mech *mech);
void mech_pilot_dbref_set(Mech *mech, DbRef pilot);
DbRef mech_gunner_dbref(const Mech *mech);
int mech_pilot_status(const Mech *mech);
void mech_pilot_status_set(Mech *mech, int status);
void mech_pilot_status_add(Mech *mech, int damage);
int mech_perception_target(const Mech *mech);
void mech_perception_target_set(Mech *mech, int target);
int mech_communication_skill(const Mech *mech);
int mech_communication_last_tick(const Mech *mech);
void mech_communication_last_tick_set(Mech *mech, int tick);
