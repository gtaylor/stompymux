#pragma once

#include <stdbool.h>

bool weapon_catalogue_is_artillery(int weapon_index);
bool weapon_catalogue_supports_indirect_fire(int weapon_index);
bool weapon_catalogue_is_anti_missile(int weapon_index);
bool weapon_catalogue_is_hot_loaded(int weapon_index, int fire_mode);
const char *weapon_catalogue_name(int weapon_index);
int weapon_catalogue_damage(int weapon_index);
int weapon_catalogue_cluster_size(int weapon_index);
