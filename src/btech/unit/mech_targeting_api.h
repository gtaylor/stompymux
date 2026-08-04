#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

void mech_targeting_lock_modes_clear(Mech *mech);
void mech_targeting_aim_reset(Mech *mech);
void mech_targeting_target_clear(Mech *mech);
DbRef mech_target_dbref(const Mech *mech);
void mech_targeting_tag_clear(Mech *mech);
bool mech_targeting_has_lock_on(const Mech *mech, DbRef target);
bool mech_targeting_lock_modes_active(const Mech *mech);
bool mech_targeting_has_specific_aim(const Mech *mech);
bool mech_movement_modes_locked(const Mech *mech);
bool mech_is_dodging(const Mech *mech);
void mech_digging_clear(Mech *mech);
