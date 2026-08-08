#pragma once

#include <stdbool.h>

typedef struct Mech Mech;

typedef struct WeaponRangeProfile {
  int minimum;
  int short_range;
  int medium_range;
  int long_range;
  int water_minimum;
  int water_short_range;
  int water_medium_range;
  int water_long_range;
} WeaponRangeProfile;

bool weapon_catalogue_is_artillery(int weapon_index);
bool weapon_catalogue_is_missile(int weapon_index);
bool weapon_catalogue_is_ballistic(int weapon_index);
bool weapon_catalogue_is_energy(int weapon_index);
bool weapon_catalogue_is_hand_to_hand(int weapon_index);
bool weapon_catalogue_is_flamer(int weapon_index);
bool weapon_catalogue_is_coolant(int weapon_index);
bool weapon_catalogue_is_acid(int weapon_index);
bool weapon_catalogue_supports_indirect_fire(int weapon_index);
bool weapon_catalogue_is_anti_missile(int weapon_index);
bool weapon_catalogue_is_personal_combat(int weapon_index);
bool weapon_catalogue_is_gauss(int weapon_index);
bool weapon_catalogue_does_not_explode(int weapon_index);
bool weapon_catalogue_is_narc(int weapon_index);
bool weapon_catalogue_is_inarc(int weapon_index);
bool weapon_catalogue_is_clan_anti_missile(int weapon_index);
bool weapon_catalogue_is_pulse(int weapon_index);
bool weapon_catalogue_is_mrm(int weapon_index);
bool weapon_catalogue_is_heavy(int weapon_index);
bool weapon_catalogue_is_rocket(int weapon_index);
bool weapon_catalogue_is_only_rocket(int weapon_index);
bool weapon_catalogue_is_dead_fire_missile(int weapon_index);
bool weapon_catalogue_is_extended_lrm(int weapon_index);
bool weapon_catalogue_is_streak(int weapon_index);
bool weapon_catalogue_is_rotary_autocannon(int weapon_index);
bool weapon_catalogue_is_heavy_gauss(int weapon_index);
bool weapon_catalogue_is_snub_ppc(int weapon_index);
bool weapon_catalogue_can_ignite_terrain(int weapon_index);
bool weapon_catalogue_can_clear_terrain(int weapon_index);
bool weapon_catalogue_is_terrain_flamer(int weapon_index);
int weapon_catalogue_personal_combat_flags(int weapon_index);
int weapon_catalogue_type(int weapon_index);
long weapon_catalogue_specials(int weapon_index);
bool weapon_catalogue_has_special(int weapon_index, int special);
bool equipment_can_use_targeting_computer(int equipment_index);
bool weapon_catalogue_is_hot_loaded(int weapon_index, int fire_mode);
const char *weapon_catalogue_name(int weapon_index);
int weapon_catalogue_damage(int weapon_index);
int weapon_catalogue_heat(int weapon_index);
int weapon_catalogue_recycle_time(int weapon_index);
int weapon_catalogue_ammunition_per_ton(int weapon_index);
int weapon_catalogue_explosion_damage(int weapon_index);
int weapon_catalogue_weight(int weapon_index);
int weapon_catalogue_cost(int weapon_index);
int weapon_catalogue_ammunition_cost(int weapon_index);
int weapon_catalogue_battle_value(int weapon_index);
int weapon_catalogue_ammunition_battle_value(int weapon_index);
int weapon_catalogue_critical_slots(int weapon_index);
WeaponRangeProfile weapon_catalogue_ranges(int weapon_index);
int weapon_catalogue_cluster_size(int weapon_index);
int weapon_catalogue_effective_range(int weapon_index, bool extended);
int weapon_catalogue_effective_water_range(int weapon_index, bool extended);
int weapon_catalogue_range_for_section(const Mech *mech, int section,
                                       int weapon_index, bool extended);
