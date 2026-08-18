/** @file
 * Public lifecycle operations for BTech special objects.
 */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "btech/ids.h"

typedef struct BtechContext BtechContext;

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

/**
 * Disposes of one BTech special object.
 *
 * @param[in] action Object and actor describing the disposal.
 */
void btech_special_object_dispose(const BtechSpecialObjectAction *action);

/**
 * Applies a special-object flag transition.
 *
 * @param[in,out] context BTech runtime context.
 * @param[in] player Object requesting the flag change.
 * @param[in] object Object whose flag changed.
 * @param[in] from Previous flag state.
 * @param[in] to New flag state.
 */
void btech_special_object_flag_changed(BtechContext *context,
                                       BtechObjectId player,
                                       BtechObjectId object, bool from,
                                       bool to);

/**
 * Checks whether an object's BTech special type may be changed.
 *
 * @param[in] context BTech runtime context.
 * @param[in] object Object to inspect.
 * @param[in] type Requested special-object type.
 * @param[out] error Buffer receiving a rejection message.
 * @param[in] error_size Capacity of @p error in bytes.
 * @return `true` when the type may be set; otherwise `false`.
 */
bool btech_special_object_type_can_set(BtechContext *context,
                                       BtechObjectId object, const char *type,
                                       char *error, size_t error_size);

/**
 * Registers one object under its configured BTech special type.
 *
 * @param[in] action Object and actor describing the registration.
 */
void btech_special_object_type_register(const BtechSpecialObjectAction *action);
