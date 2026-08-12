
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
extern void mechrep_rloadnew2(DbRef player, void *data, char *buffer);
extern void mechrep_rrestock(DbRef player, void *data, char *buffer);
extern void mechrep_rfiremode(DbRef player, void *data, char *buffer);
extern void mechrep_rsettech(DbRef player, void *data, char *buffer);
