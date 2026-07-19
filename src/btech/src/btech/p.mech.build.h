
/*
   p.mech.build.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:45 CET 1999 from mech.build.c */

#pragma once

typedef struct BtechContext BtechContext;

typedef struct ArmorSectionAbbreviation {
  char text[4];
} ArmorSectionAbbreviation;

/* mech.build.c */
void FillDefaultCriticals(MECH *mech, int index);
ArmorSectionAbbreviation armor_section_abbreviation(char type, char mtype,
                                                    int loc);
int ArmorSectionFromString(char type, char mtype, char *string);
int WeaponIndexFromString(BtechContext *context, char *string);
int FindSpecialItemCodeFromString(BtechContext *context, char *buffer);
