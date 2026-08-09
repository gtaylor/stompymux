
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
extern void mechrep_Raddspecial(DbRef player, void *data, char *buffer);
extern void mechrep_Raddtech(DbRef player, void *data, char *buffer);
extern void mechrep_Raddinftech(DbRef player, void *data, char *buffer);
extern void mechrep_Raddweap(DbRef player, void *data, char *buffer);
extern void mechrep_Rdeltech(DbRef player, void *data, char *buffer);
extern void mechrep_Rdelinftech(DbRef player, void *data, char *buffer);
extern void mechrep_Rdisplaysection(DbRef player, void *data, char *buffer);
extern void mechrep_Rloadnew(DbRef player, void *data, char *buffer);
extern void mechrep_Rloadnew2(DbRef player, void *data, char *buffer);
extern void mechrep_Rreload(DbRef player, void *data, char *buffer);
extern void mechrep_Rrestock(DbRef player, void *data, char *buffer);
extern void mechrep_Rfiremode(DbRef player, void *data, char *buffer);
extern void mechrep_Rrepair(DbRef player, void *data, char *buffer);
extern void mechrep_Rresetcrits(DbRef player, void *data, char *buffer);
extern void mechrep_Rrestore(DbRef player, void *data, char *buffer);
extern void mechrep_Rsavetemp(DbRef player, void *data, char *buffer);
extern void mechrep_Rsavetemp2(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetarmor(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetheatsinks(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetjumpspeed(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetlrsrange(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetmove(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetradio(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetradiorange(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetscanrange(DbRef player, void *data, char *buffer);
extern void mechrep_Rsetspeed(DbRef player, void *data, char *buffer);
extern void mechrep_Rsettacrange(DbRef player, void *data, char *buffer);
extern void mechrep_Rsettarget(DbRef player, void *data, char *buffer);
extern void mechrep_Rsettech(DbRef player, void *data, char *buffer);
extern void mechrep_Rsettons(DbRef player, void *data, char *buffer);
extern void mechrep_Rsettype(DbRef player, void *data, char *buffer);
extern void mechrep_Rshowtech(DbRef player, void *data, char *buffer);

/* Mem alloc/free routines */
void newfreemechrep(DbRef key, void **data,
                    BtechSpecialLifecycleOperation operation);
