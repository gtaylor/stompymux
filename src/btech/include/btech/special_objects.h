/** @file
 * Public lifecycle operations for BTech special objects.
 */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "btech/ids.h"

typedef struct BtechContext BtechContext;

/** Durable kinds of registered BTech special object. */
typedef enum BtechSpecialObjectType : int {
  BTECH_SPECIAL_MECH,
  BTECH_SPECIAL_DEBUG,
  BTECH_SPECIAL_MAP,
  BTECH_SPECIAL_AUTOPILOT,
  BTECH_SPECIAL_TURRET,
} BtechSpecialObjectType;

/** Context for an operation on one BTech special object. */
typedef struct BtechSpecialObjectAction {
  /** BTech runtime context. */
  BtechContext *context;
  /** Object performing the operation. */
  BtechObjectId actor;
  /** Object affected by the operation. */
  BtechObjectId object;
} BtechSpecialObjectAction;

/**
 * Loads all BTech special objects into the runtime registries.
 *
 * @param[in,out] context BTech runtime context.
 */
void btech_special_objects_load(BtechContext *context);

/**
 * Updates all active BTech special objects.
 *
 * @param[in,out] context BTech runtime context.
 */
void btech_special_objects_update(BtechContext *context);

/**
 * Clears all BTech special-object runtime state.
 *
 * @param[in,out] context BTech runtime context.
 */
void btech_special_objects_reset(BtechContext *context);

/** Returns the registered type, or `-1` when the object is not registered. */
int btech_special_object_type(BtechContext *context, BtechObjectId object);

/** Returns the stable uppercase name for a registered type. */
const char *btech_special_object_type_name(int type);

/** Registers a live controlled thing as one BTech special-object type. */
bool btech_special_object_register(BtechContext *context, BtechObjectId actor,
                                   BtechObjectId object, const char *type,
                                   char *error, size_t error_size);

/** Unregisters a controlled BTech special object. */
bool btech_special_object_unregister(BtechContext *context, BtechObjectId actor,
                                     BtechObjectId object, char *error,
                                     size_t error_size);

/** Forgets all runtime and configuration state owned by one object. */
void btech_object_forget(BtechContext *context, BtechObjectId object);

/**
 * Disposes of one BTech special object.
 *
 * @param[in] action Object and actor describing the disposal.
 */
void btech_special_object_dispose(const BtechSpecialObjectAction *action);
