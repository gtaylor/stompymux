/* Defines BattleTech special-object types and lifecycle callbacks. */

#pragma once

#include <stddef.h>

typedef struct BtechContext BtechContext;

typedef enum {
  GTYPE_MECH,
  GTYPE_DEBUG,
  GTYPE_MECHREP,
  GTYPE_MAP,
  GTYPE_AUTO,
  GTYPE_TURRET,
  GTYPE_UNUSED1 /* placeholder for old chargen object */
} BtechSpecialObjectType;

typedef enum BtechSpecialLifecycleOperation : int {
  SPECIAL_FREE = 0,
  SPECIAL_ALLOC = 1,
} BtechSpecialLifecycleOperation;

/*
 * Base "class" for all XCODE objects.  Every XCODE object should start with a
 * field of this type, called 'xcode' by convention.
 */
typedef struct BtechSpecialObject {
  BtechSpecialObjectType type; /* XCODE object type */
  size_t size;                 /* object size */
  BtechContext *context;       /* borrowed runtime owner */
} BtechSpecialObject;
