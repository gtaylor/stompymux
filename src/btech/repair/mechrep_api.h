/* Declares the BattleTech mechrep API. */

#include "mux/server/platform.h"
#include "special_object.h"

#pragma once

typedef struct BtechContext BtechContext;

/* mechrep.c */
void newfreemechrep(DbRef key, void **data,
                    BtechSpecialLifecycleOperation selector);
void mechrep_rresetcrits(DbRef player, void *data, char *buffer);
void mechrep_rdisplaysection(DbRef player, void *data, char *buffer);
void mechrep_rsetradio(DbRef player, void *data, char *buffer);
void mechrep_rsettarget(DbRef player, void *data, char *buffer);
void mechrep_rsettype(DbRef player, void *data, char *buffer);
void mechrep_rsetspeed(DbRef player, void *data, char *buffer);
void mechrep_rsetjumpspeed(DbRef player, void *data, char *buffer);
void mechrep_rsetheatsinks(DbRef player, void *data, char *buffer);
void mechrep_rsetlrsrange(DbRef player, void *data, char *buffer);
void mechrep_rsettacrange(DbRef player, void *data, char *buffer);
void mechrep_rsetscanrange(DbRef player, void *data, char *buffer);
void mechrep_rsetradiorange(DbRef player, void *data, char *buffer);
void mechrep_rsettons(DbRef player, void *data, char *buffer);
void mechrep_rsetmove(DbRef player, void *data, char *buffer);
void mechrep_rloadnew(DbRef player, void *data, char *buffer);
Mech *load_refmech(BtechContext *context, const char *reference);
void mech_reference_cache_destroy(BtechContext *context);
void mechrep_rrestore(DbRef player, void *data, char *buffer);
void mechrep_rsavetemp(DbRef player, void *data, char *buffer);
void mechrep_rsavetemp2(DbRef player, void *data, char *buffer);
void mechrep_rsetarmor(DbRef player, void *data, char *buffer);
void mechrep_raddweap(DbRef player, void *data, char *buffer);
void mechrep_rreload(DbRef player, void *data, char *buffer);
void mechrep_rrepair(DbRef player, void *data, char *buffer);
void mechrep_raddspecial(DbRef player, void *data, char *buffer);
const char *techstatus_func(Mech *mech);
void mechrep_rshowtech(DbRef player, void *data, char *buffer);
void mechrep_gettechstring(Mech *mech, char *buffer);
void mechrep_rdeltech(DbRef player, void *data, char *buffer);
void mechrep_raddtech(DbRef player, void *data, char *buffer);
void mechrep_rdelinftech(DbRef player, void *data, char *buffer);
void mechrep_raddinftech(DbRef player, void *data, char *buffer);
void mechrep_setcargospace(DbRef player, void *data, char *buffer);
void invalid_section(DbRef player, Mech *mech);
