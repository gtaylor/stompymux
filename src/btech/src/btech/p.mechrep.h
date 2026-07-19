
/*
   p.mechrep.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Tue Feb  9 14:31:38 CET 1999 from mechrep.c */

#include "mux/server/platform.h"

#pragma once

typedef struct BtechContext BtechContext;

/* mechrep.c */
void newfreemechrep(DbRef key, void **data, int selector);
void mechrep_Rresetcrits(DbRef player, void *data, char *buffer);
void mechrep_Rdisplaysection(DbRef player, void *data, char *buffer);
void mechrep_Rsetradio(DbRef player, void *data, char *buffer);
void mechrep_Rsettarget(DbRef player, void *data, char *buffer);
void mechrep_Rsettype(DbRef player, void *data, char *buffer);
void mechrep_Rsetspeed(DbRef player, void *data, char *buffer);
void mechrep_Rsetjumpspeed(DbRef player, void *data, char *buffer);
void mechrep_Rsetheatsinks(DbRef player, void *data, char *buffer);
void mechrep_Rsetlrsrange(DbRef player, void *data, char *buffer);
void mechrep_Rsettacrange(DbRef player, void *data, char *buffer);
void mechrep_Rsetscanrange(DbRef player, void *data, char *buffer);
void mechrep_Rsetradiorange(DbRef player, void *data, char *buffer);
void mechrep_Rsettons(DbRef player, void *data, char *buffer);
void mechrep_Rsetmove(DbRef player, void *data, char *buffer);
void mechrep_Rloadnew(DbRef player, void *data, char *buffer);
void clear_mech(MECH *mech, int flag);
char *mechref_path(BtechContext *context, const char *mech_path, char *id);
void mech_template_registry_destroy(BtechContext *context);
int load_mechdata2(DbRef player, MECH *mech, char *id);
int unable_to_find_proper_type(int i);
int load_mechdata(MECH *mech, char *id);
int mech_loadnew(DbRef player, MECH *mech, char *id);
MECH *load_refmech(BtechContext *context, const char *reference);
void mech_reference_cache_destroy(BtechContext *context);
void mechrep_Rrestore(DbRef player, void *data, char *buffer);
void mechrep_Rsavetemp(DbRef player, void *data, char *buffer);
void mechrep_Rsavetemp2(DbRef player, void *data, char *buffer);
void mechrep_Rsetarmor(DbRef player, void *data, char *buffer);
void mechrep_Raddweap(DbRef player, void *data, char *buffer);
void mechrep_Rreload(DbRef player, void *data, char *buffer);
void mechrep_Rrepair(DbRef player, void *data, char *buffer);
void mechrep_Raddspecial(DbRef player, void *data, char *buffer);
char *techstatus_func(MECH *mech);
void mechrep_Rshowtech(DbRef player, void *data, char *buffer);
void mechrep_gettechstring(MECH *mech, char *buffer);
void mechrep_Rdeltech(DbRef player, void *data, char *buffer);
void mechrep_Raddtech(DbRef player, void *data, char *buffer);
void mechrep_Rdelinftech(DbRef player, void *data, char *buffer);
void mechrep_Raddinftech(DbRef player, void *data, char *buffer);
void mechrep_setcargospace(DbRef player, void *data, char *buffer);
void invalid_section(DbRef player, MECH *mech);
