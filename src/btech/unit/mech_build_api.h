/* Declares the BattleTech unit build API. */

#pragma once

#include "mux/server/platform.h"
#include "section_types.h"

typedef struct BtechContext BtechContext;

typedef struct ArmorSectionAbbreviation {
  char text[4];
} ArmorSectionAbbreviation;

/* mech.build.c */
void FillDefaultCriticals(Mech *mech, int index);
ArmorSectionAbbreviation
armor_section_abbreviation(UnitClass type, MechMovementType movement_type,
                           int location);
int ArmorSectionFromString(UnitClass type, MechMovementType movement_type,
                           const char *string);
int WeaponIndexFromString(BtechContext *context, char *string);
int FindSpecialItemCodeFromString(BtechContext *context, char *buffer);
