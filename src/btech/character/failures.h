
/* Defines failure types and modifiers for BattleTech units. */

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

typedef struct PartFailureCall {
  Mech *mech;
  int weapon_number;
  int weapon_type;
  int section;
  int critical;
  int roll;
} PartFailureCall;

typedef struct PartFailureResult {
  int type;
  int modifier;
} PartFailureResult;

typedef PartFailureResult (*PartFailureHandler)(const PartFailureCall *call);

typedef struct PartFailure {
  const char *message;
  int data; /* things like percent to alter */
  PartFailureHandler handler;
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
