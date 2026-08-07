#pragma once

#include <stdbool.h>

typedef struct Mech Mech;

bool weapon_catalogue_is_artillery(int weapon_index);
bool weapon_catalogue_is_missile(int weapon_index);
bool weapon_catalogue_is_ballistic(int weapon_index);
bool weapon_catalogue_is_energy(int weapon_index);
bool weapon_catalogue_is_flamer(int weapon_index);
bool weapon_catalogue_is_coolant(int weapon_index);
bool weapon_catalogue_is_acid(int weapon_index);
bool weapon_catalogue_supports_indirect_fire(int weapon_index);
bool weapon_catalogue_is_anti_missile(int weapon_index);
bool equipment_can_use_targeting_computer(int equipment_index);
bool weapon_catalogue_is_hot_loaded(int weapon_index, int fire_mode);
const char *weapon_catalogue_name(int weapon_index);
int weapon_catalogue_damage(int weapon_index);
int weapon_catalogue_cluster_size(int weapon_index);
int weapon_catalogue_effective_range(int weapon_index, bool extended);
int weapon_catalogue_effective_water_range(int weapon_index, bool extended);
int weapon_catalogue_range_for_section(const Mech *mech, int section,
                                       int weapon_index, bool extended);
