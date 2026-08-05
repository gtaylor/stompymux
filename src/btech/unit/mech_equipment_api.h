#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_critical_part_type(const Mech *mech, int section, int critical);
int mech_critical_brand(const Mech *mech, int section, int critical);
int mech_critical_data(const Mech *mech, int section, int critical);
int mech_critical_fire_mode(const Mech *mech, int section, int critical);
int mech_critical_ammo_mode(const Mech *mech, int section, int critical);
int mech_critical_temporary_failure(const Mech *mech, int section,
                                    int critical);
int mech_critical_full_ammunition(const Mech *mech, int section, int critical);
bool mech_critical_is_nonfunctional(const Mech *mech, int section,
                                    int critical);
bool mech_critical_is_disabled(const Mech *mech, int section, int critical);
bool mech_critical_is_destroyed(const Mech *mech, int section, int critical);
bool mech_critical_is_broken(const Mech *mech, int section, int critical);
bool mech_critical_is_damaged(const Mech *mech, int section, int critical);
void mech_critical_temporary_failure_set(Mech *mech, int section, int critical,
                                         int failure);
void mech_critical_data_set(Mech *mech, int section, int critical, int data);
void mech_critical_part_type_set(Mech *mech, int section, int critical,
                                 int part_type);
void mech_critical_destroyed_set(Mech *mech, int section, int critical,
                                 bool destroyed);
int mech_section_original_armor(const Mech *mech, int section);
int mech_section_original_rear_armor(const Mech *mech, int section);
int mech_section_armor(const Mech *mech, int section);
int mech_section_rear_armor(const Mech *mech, int section);
int mech_section_internal(const Mech *mech, int section);
int mech_section_original_internal(const Mech *mech, int section);
bool mech_section_is_destroyed(const Mech *mech, int section);
bool mech_section_is_flooded(const Mech *mech, int section);
void mech_section_flooded_set(Mech *mech, int section, bool flooded);
bool mech_critical_is_operational_special(const Mech *mech, int section,
                                          int critical, int special);
bool mech_section_carries_club(const Mech *mech, int section);
bool mech_has_attached_inarc_ecm(const Mech *mech);
bool mech_has_attached_homing_beacon(const Mech *mech);
bool mech_limbs_are_recycling(const Mech *mech);
bool mech_weapon_is_recycling_at(const Mech *mech, int section, int critical);
bool mech_weapon_is_nonfunctional_at(Mech *mech, int section, int critical,
                                     int weapon_index);
int mech_section_recycle_ticks(const Mech *mech, int section);
bool mech_part_is_structural_placeholder(int part_type);
void mech_section_armor_set(Mech *mech, int section, int armor);
void mech_section_rear_armor_set(Mech *mech, int section, int armor);
void mech_section_original_armor_set(Mech *mech, int section, int armor);
void mech_section_internal_set(Mech *mech, int section, int internal);
void mech_section_original_internal_set(Mech *mech, int section, int internal);
void mech_critical_configure(Mech *mech, int section, int critical,
                             int part_type, int data, int fire_mode,
                             int ammo_mode);
