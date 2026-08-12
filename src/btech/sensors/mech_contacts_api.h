/* Declares the BattleTech unit contacts API. */

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
MechStatusString mech_status_string(Mech *target, int who);
char mech_contact_status_character(Mech *mech, Mech *target, int w_char_num);
