
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

/* these are types of modifiers */
typedef enum FailureModifierType : int {
  HEAT = 1,
  RANGE = 2,
  DAMAGE = 3,
  POWER_SPIKE = 4,
  WEAPON_JAMMED = 5,
  WEAPON_DUD = 6,
  CRAZY_MISSILES = 7,
} FailureModifierType;

typedef enum GenericFailureType : int {
  FAIL_STATIC = 1,
} GenericFailureType;

/* these are catagories of damage */
typedef enum TemporaryFailureStatus : int {
  FAIL_NONE = 0,
  FAIL_JAMMED = 1,
  FAIL_SHORTED = 2,
  FAIL_DUD = 3,
  FAIL_EMPTY = 4,
  FAIL_DESTROYED = 5,
  FAIL_AMMOJAMMED = 6,
  FAIL_AMMOCRITJAMMED = 7,
} TemporaryFailureStatus;

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
