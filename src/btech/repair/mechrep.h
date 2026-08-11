
/* Declares common unit repair interfaces. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"
#include "special_object.h"

/* This is the silly structure that I use for the repair stuff */
typedef struct RepairFacility {
  BtechSpecialObject xcode; /* XCODE base class field */

  DbRef mynum;
  DbRef current_target;
} RepairFacility;

/* Mech repair/type commands */
extern void mechrep_raddspecial(DbRef player, void *data, char *buffer);
extern void mechrep_raddtech(DbRef player, void *data, char *buffer);
extern void mechrep_raddinftech(DbRef player, void *data, char *buffer);
extern void mechrep_raddweap(DbRef player, void *data, char *buffer);
extern void mechrep_rdeltech(DbRef player, void *data, char *buffer);
extern void mechrep_rdelinftech(DbRef player, void *data, char *buffer);
extern void mechrep_rdisplaysection(DbRef player, void *data, char *buffer);
extern void mechrep_rloadnew(DbRef player, void *data, char *buffer);
extern void mechrep_rloadnew2(DbRef player, void *data, char *buffer);
extern void mechrep_rreload(DbRef player, void *data, char *buffer);
extern void mechrep_rrestock(DbRef player, void *data, char *buffer);
extern void mechrep_rfiremode(DbRef player, void *data, char *buffer);
extern void mechrep_rrepair(DbRef player, void *data, char *buffer);
extern void mechrep_rresetcrits(DbRef player, void *data, char *buffer);
extern void mechrep_rrestore(DbRef player, void *data, char *buffer);
extern void mechrep_rsavetemp(DbRef player, void *data, char *buffer);
extern void mechrep_rsavetemp2(DbRef player, void *data, char *buffer);
extern void mechrep_rsetarmor(DbRef player, void *data, char *buffer);
extern void mechrep_rsetheatsinks(DbRef player, void *data, char *buffer);
extern void mechrep_rsetjumpspeed(DbRef player, void *data, char *buffer);
extern void mechrep_rsetlrsrange(DbRef player, void *data, char *buffer);
extern void mechrep_rsetmove(DbRef player, void *data, char *buffer);
extern void mechrep_rsetradio(DbRef player, void *data, char *buffer);
extern void mechrep_rsetradiorange(DbRef player, void *data, char *buffer);
extern void mechrep_rsetscanrange(DbRef player, void *data, char *buffer);
extern void mechrep_rsetspeed(DbRef player, void *data, char *buffer);
extern void mechrep_rsettacrange(DbRef player, void *data, char *buffer);
extern void mechrep_rsettarget(DbRef player, void *data, char *buffer);
extern void mechrep_rsettech(DbRef player, void *data, char *buffer);
extern void mechrep_rsettons(DbRef player, void *data, char *buffer);
extern void mechrep_rsettype(DbRef player, void *data, char *buffer);
extern void mechrep_rshowtech(DbRef player, void *data, char *buffer);

/* Mem alloc/free routines */
void newfreemechrep(DbRef key, void **data,
                    BtechSpecialLifecycleOperation operation);
