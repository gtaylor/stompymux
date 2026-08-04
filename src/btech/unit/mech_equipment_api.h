#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_critical_part_type(const Mech *mech, int section, int critical);
int mech_critical_brand(const Mech *mech, int section, int critical);
int mech_critical_data(const Mech *mech, int section, int critical);
int mech_critical_fire_mode(const Mech *mech, int section, int critical);
int mech_critical_ammo_mode(const Mech *mech, int section, int critical);
void mech_critical_temporary_failure_set(Mech *mech, int section, int critical,
                                         int failure);
int mech_section_original_armor(const Mech *mech, int section);
int mech_section_original_rear_armor(const Mech *mech, int section);
int mech_section_armor(const Mech *mech, int section);
int mech_section_rear_armor(const Mech *mech, int section);
int mech_section_internal(const Mech *mech, int section);
int mech_section_original_internal(const Mech *mech, int section);
bool mech_section_is_destroyed(const Mech *mech, int section);
bool mech_critical_is_operational_special(const Mech *mech, int section,
                                          int critical, int special);
bool mech_section_carries_club(const Mech *mech, int section);
void mech_section_armor_set(Mech *mech, int section, int armor);
void mech_section_original_armor_set(Mech *mech, int section, int armor);
void mech_section_internal_set(Mech *mech, int section, int internal);
void mech_section_original_internal_set(Mech *mech, int section, int internal);
void mech_critical_configure(Mech *mech, int section, int critical,
                             int part_type, int data, int fire_mode,
                             int ammo_mode);
