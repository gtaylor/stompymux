/* Defines BattleTech special-object types and lifecycle callbacks. */

#pragma once

#include <stddef.h>

typedef struct BtechContext BtechContext;
typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

#include "btech/special_objects.h"

enum : int {
  GTYPE_MECH = BTECH_SPECIAL_MECH,
  GTYPE_DEBUG = BTECH_SPECIAL_DEBUG,
  GTYPE_MAP = BTECH_SPECIAL_MAP,
  GTYPE_AUTO = BTECH_SPECIAL_AUTOPILOT,
  GTYPE_TURRET = BTECH_SPECIAL_TURRET,
};

typedef enum BtechSpecialLifecycleOperation : int {
  SPECIAL_FREE = 0,
  SPECIAL_ALLOC = 1,
} BtechSpecialLifecycleOperation;

/*
 * Base "class" for registered BTech objects. Concrete legacy structs start
 * with a field of this type, still called `xcode` for layout compatibility.
 */
typedef struct BtechSpecialObject {
  BtechSpecialObjectType type;
  size_t size;           /* object size */
  BtechContext *context; /* borrowed runtime owner */
} BtechSpecialObject;

/* Checked downcasts for objects already retrieved from the special registry. */
static inline Mech *btech_special_object_as_mech(BtechSpecialObject *object) {
  if (object == nullptr || object->type != GTYPE_MECH)
    return nullptr;
  return (Mech *)object;
}

static inline BattleMap *
btech_special_object_as_map(BtechSpecialObject *object) {
  if (object == nullptr || object->type != GTYPE_MAP)
    return nullptr;
  return (BattleMap *)object;
}
