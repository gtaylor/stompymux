
/* Brand level modifiers and failure resultant data here after included.
   Failure.h
   Created By: Nim
   Dated:      9 - 21 - 96

   Parts copyright (c) 2002 Thomas Wouters

   $Id: failures.h,v 1.1.1.1 2005/01/11 21:18:07 kstevens Exp $
   Last modified: Sat Jun  6 20:27:26 1998 fingon
 */

#pragma once

#include "mech_api_types.h"
#include "mech_lifecycle.h"

#define IsAutocannon(a) (MechWeapons[a].type == TAMMO)
#define IsEnergy(a) (MechWeapons[a].type == TBEAM)
/*#define IsFlamer(a) (MechWeapons[a].type==TBEAM && \
        strstr(MechWeapons[a].name, "Flamer")) */

/* these are types of modifiers */
#define HEAT 1
#define RANGE 2
#define DAMAGE 3
#define POWER_SPIKE 4
#define WEAPON_JAMMED 5
#define WEAPON_DUD 6
#define CRAZY_MISSILES 7

#define FAIL_STATIC 1

/* these are catagories of damage */
#define FAIL_NONE 0
#define FAIL_JAMMED 1
#define FAIL_SHORTED 2
#define FAIL_DUD 3
#define FAIL_EMPTY 4
#define FAIL_DESTROYED 5
#define FAIL_AMMOJAMMED 6
#define FAIL_AMMOCRITJAMMED 7

typedef struct PartBrand {
  const char *name;
  short level;
  int success;
  int modifier;
} PartBrand;

typedef struct PartFailure {
  const char *message;
  int data; /* things like percent to alter */
  void (*func)(Mech *, int, int, int, int, int, int *, int *);
  int type;
  int flag;
} PartFailure;

/*  Brand keys
   1 - This is absolute crap.
   2 - This is low end.
   3 - This is average.
   4 - These are supieror parts.
   5 - These are EXTREMELY RARE and EXTREMELY reliable
 */
