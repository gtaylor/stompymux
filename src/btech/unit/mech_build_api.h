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
void FillDefaultCriticals(Mech *mech, int index);
ArmorSectionAbbreviation
armor_section_abbreviation(const ArmorSectionReference *section);
int ArmorSectionFromString(UnitClass type, MechMovementType movement_type,
                           const char *string);
int WeaponIndexFromString(BtechContext *context, char *string);
int FindSpecialItemCodeFromString(BtechContext *context, char *buffer);
