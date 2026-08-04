#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_movement_type(const Mech *mech);
int mech_tonnage(const Mech *mech);
int mech_engine_rating(const Mech *mech);
float mech_jump_speed(const Mech *mech);
int mech_heat_sink_count(const Mech *mech);
int mech_technology_flags(const Mech *mech);
int mech_technology_flags_secondary(const Mech *mech);
float mech_current_speed(const Mech *mech);
float mech_maximum_speed(const Mech *mech);
void mech_maximum_speed_set(Mech *mech, float speed);
bool mech_is_flying_type(const Mech *mech);
bool mech_is_omni(const Mech *mech);
int mech_fuel(const Mech *mech);
int mech_original_fuel(const Mech *mech);
int mech_structural_integrity(const Mech *mech);
int mech_original_structural_integrity(const Mech *mech);
void mech_maximum_fuel_set(Mech *mech, int fuel);
void mech_cargo_weight_set(Mech *mech, int weight);
