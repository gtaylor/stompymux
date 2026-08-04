#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

int mech_critical_part_type(const Mech *mech, int section, int critical);
int mech_section_original_armor(const Mech *mech, int section);
int mech_section_original_rear_armor(const Mech *mech, int section);
bool mech_section_is_destroyed(const Mech *mech, int section);
void mech_section_armor_set(Mech *mech, int section, int armor);
void mech_section_original_armor_set(Mech *mech, int section, int armor);
void mech_section_internal_set(Mech *mech, int section, int internal);
void mech_section_original_internal_set(Mech *mech, int section, int internal);
void mech_critical_configure(Mech *mech, int section, int critical,
                             int part_type, int data, int fire_mode,
                             int ammo_mode);
