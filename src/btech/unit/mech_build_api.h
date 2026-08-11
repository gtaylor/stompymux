/* Declares the BattleTech unit build API. */

#pragma once

#include "mux/server/platform.h"
#include "section_types.h"

typedef struct BtechContext BtechContext;

typedef struct ArmorSectionAbbreviation {
  char text[4];
} ArmorSectionAbbreviation;

typedef struct ArmorSectionReference {
  UnitClass unit_class;
  MechMovementType movement_type;
  int location;
} ArmorSectionReference;

/* mech.build.c */
void fill_default_criticals(Mech *mech, int index);
ArmorSectionAbbreviation
armor_section_abbreviation(const ArmorSectionReference *section);
int armor_section_from_string(UnitClass type, MechMovementType movement_type,
                              const char *string);
int weapon_index_from_string(BtechContext *context, char *string);
int find_special_item_code_from_string(BtechContext *context, char *buffer);
