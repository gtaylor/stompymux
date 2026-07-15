
/*
   p.mech.contacts.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:47 CET 1999 from mech.contacts.c */

#pragma once

/* mech.contacts.c */
void show_brief_flags(DbRef player, MECH *mech);
void mech_brief(DbRef player, void *data, char *buffer);
void mech_contacts(DbRef player, void *data, char *buffer);
char getWeaponArc(MECH *mech, int arc);
char *getStatusString(MECH *target, int enemy);
char getStatusChar(MECH *mech, MECH *mechTarget, int wCharNum);
