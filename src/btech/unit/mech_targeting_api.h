#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

void mech_targeting_lock_modes_clear(Mech *mech);
void mech_targeting_aim_reset(Mech *mech);
void mech_targeting_target_clear(Mech *mech);
DbRef mech_target_dbref(const Mech *mech);
DbRef mech_charge_target_dbref(const Mech *mech);
DbRef mech_dfa_target_dbref(const Mech *mech);
int mech_charge_timer(const Mech *mech);
int mech_target_hex_x(const Mech *mech);
int mech_target_hex_y(const Mech *mech);
int mech_target_hex_z(const Mech *mech);
DbRef mech_spotter_dbref(const Mech *mech);
void mech_spotter_dbref_set(Mech *mech, DbRef spotter);
void mech_fire_adjustment_set(Mech *mech, int adjustment);
int mech_targeting_computer_type(const Mech *mech);
int mech_aim_section(const Mech *mech);
int mech_aim_unit_class(const Mech *mech);
bool mech_targets_building(const Mech *mech);
bool mech_targets_hex(const Mech *mech);
void mech_targeting_tag_clear(Mech *mech);
bool mech_targeting_has_lock_on(const Mech *mech, DbRef target);
bool mech_targeting_lock_modes_active(const Mech *mech);
bool mech_targeting_has_specific_aim(const Mech *mech);
bool mech_movement_modes_locked(const Mech *mech);
bool mech_is_dodging(const Mech *mech);
void mech_digging_clear(Mech *mech);
