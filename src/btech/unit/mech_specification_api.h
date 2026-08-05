#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_movement_type(const Mech *mech);
void mech_movement_type_set(Mech *mech, int movement_type);
int mech_tonnage(const Mech *mech);
void mech_tonnage_set(Mech *mech, int tonnage);
int mech_real_tonnage(const Mech *mech);
int mech_engine_rating(const Mech *mech);
float mech_jump_speed(const Mech *mech);
void mech_jump_speed_lower(Mech *mech, float amount);
int mech_heat_sink_count(const Mech *mech);
void mech_heat_sink_count_remove(Mech *mech, int count);
void mech_heat_sink_count_add(Mech *mech, int count);
bool mech_has_double_heat_sinks(const Mech *mech);
int mech_heat_sink_critical_size(const Mech *mech);
int mech_technology_flags(const Mech *mech);
int mech_technology_flags_secondary(const Mech *mech);
void mech_technology_flags_remove(Mech *mech, int flags);
void mech_technology_flags_secondary_remove(Mech *mech, int flags);
void mech_masc_technology_destroy(Mech *mech);
void mech_supercharger_technology_destroy(Mech *mech);
int mech_infantry_technology_flags(const Mech *mech);
int mech_cargo_space(const Mech *mech);
void mech_cargo_space_remove(Mech *mech, int amount);
int mech_carrier_maximum_tonnage(const Mech *mech);
int mech_maximum_battle_suits(const Mech *mech);
float mech_current_speed(const Mech *mech);
void mech_current_speed_set(Mech *mech, float speed);
void mech_current_speed_scale(Mech *mech, float factor);
void mech_current_speed_reduce_toward_zero(Mech *mech, float amount);
float mech_maximum_speed(const Mech *mech);
float mech_template_maximum_speed(const Mech *mech);
void mech_maximum_speed_set(Mech *mech, float speed);
bool mech_is_flying_type(const Mech *mech);
bool mech_is_omni(const Mech *mech);
int mech_fuel(const Mech *mech);
void mech_fuel_set(Mech *mech, int fuel);
void mech_fuel_decrement(Mech *mech, int amount);
int mech_original_fuel(const Mech *mech);
int mech_structural_integrity(const Mech *mech);
void mech_structural_integrity_set(Mech *mech, int integrity);
int mech_original_structural_integrity(const Mech *mech);
DbRef mech_bay_dbref(const Mech *mech, int bay);
void mech_maximum_fuel_set(Mech *mech, int fuel);
void mech_cargo_weight_set(Mech *mech, int weight);
bool mech_has_sixth_sense(const Mech *mech);
void mech_sixth_sense_set(Mech *mech, bool enabled);
void mech_bay_dbref_set(Mech *mech, int bay, DbRef bay_dbref);
int mech_carried_cargo_weight(const Mech *mech);
bool mech_load_cache_is_valid(const Mech *mech);
bool mech_weight_cache_is_valid(const Mech *mech);
void mech_weight_cache_invalidate(Mech *mech);
bool mech_speed_cache_is_valid(const Mech *mech);
void mech_load_cache_invalidate(Mech *mech);
int mech_cached_calculated_weight(const Mech *mech);
void mech_cached_calculated_weight_set(Mech *mech, int weight);
int mech_cached_lugged_weight(const Mech *mech);
void mech_load_cache_record(Mech *mech, int lugged_weight);
float mech_cached_maximum_speed(const Mech *mech);
void mech_speed_cache_record(Mech *mech, float speed, int walk_xp_factor);
