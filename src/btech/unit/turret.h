
/* Declares turret control interfaces. */

#pragma once

#include "special_object.h"

#include "equipment_types.h"

typedef struct Turret {
  BtechSpecialObject xcode; /* XCODE base class field */
  DbRef mynum;

  int arcs;
  unsigned long tic[NUM_TICS]; /* tics.. */
  DbRef parent;                /* ship whose stats we use for this */
  DbRef gunner;                /* who's da gunner? */
  DbRef target;                /* what do we have locked? */
  short targx, targy, targz;   /* in map coords, target squares */
  int lockmode;                /* lock modes (hex, etc) */
} Turret;
