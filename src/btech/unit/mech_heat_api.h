#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

float mech_excess_heat(const Mech *mech);
float mech_heat_production(const Mech *mech);
float mech_heat_dissipation(const Mech *mech);
float mech_weapon_heat(const Mech *mech);
float mech_active_heat_sinks(const Mech *mech);
bool mech_uses_heat(const Mech *mech);
float mech_added_heat(const Mech *mech);
int mech_disabled_heat_sink_count(const Mech *mech);
int mech_engine_heat(const Mech *mech);
void mech_engine_heat_set(Mech *mech, int heat);
void mech_engine_heat_add(Mech *mech, int heat);
bool mech_heat_cutoff_is_enabled(const Mech *mech);
bool mech_life_support_is_destroyed(const Mech *mech);
void mech_heat_production_set(Mech *mech, float heat);
void mech_heat_production_add(Mech *mech, float heat);
void mech_heat_dissipation_set(Mech *mech, float heat);
void mech_heat_dissipation_add(Mech *mech, float heat);
void mech_excess_heat_set(Mech *mech, float heat);
void mech_weapon_heat_set(Mech *mech, float heat);
void mech_weapon_heat_add(Mech *mech, float heat);
void mech_added_heat_add(Mech *mech, float heat);
void mech_disabled_heat_sinks_set(Mech *mech, int count);
int mech_last_overheat_check_tick(const Mech *mech);
void mech_last_overheat_check_tick_set(Mech *mech, int tick);
