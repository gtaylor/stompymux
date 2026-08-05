
/*
   p.mech.contacts.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:47 CET 1999 from mech.contacts.c */

#pragma once

#include "mux/server/platform.h"

typedef struct Mech Mech;

typedef struct MechStatusString {
  char text[24];
} MechStatusString;

/* mech.contacts.c */
void show_brief_flags(DbRef player, Mech *mech);
void mech_brief(DbRef player, void *data, char *buffer);
void mech_contacts(DbRef player, void *data, char *buffer);
char mech_contact_weapon_arc(int arc);
MechStatusString mech_status_string(Mech *target, int enemy);
char mech_contact_status_character(Mech *mech, Mech *target,
                                   int character_number);
