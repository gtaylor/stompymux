/* special_objects.h - Public lifecycle operations for BTech objects. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "btech/ids.h"

typedef struct BtechContext BtechContext;

typedef struct BtechSpecialObjectAction {
  BtechContext *context;
  BtechObjectId actor;
  BtechObjectId object;
} BtechSpecialObjectAction;

void btech_special_objects_load(BtechContext *context);
void btech_special_objects_update(BtechContext *context);
void btech_special_objects_reset(BtechContext *context);
void btech_special_object_dispose(const BtechSpecialObjectAction *action);
void btech_special_object_flag_changed(BtechContext *context,
                                       BtechObjectId player,
                                       BtechObjectId object, bool from,
                                       bool to);
bool btech_special_object_type_can_set(BtechContext *context,
                                       BtechObjectId object, const char *type,
                                       char *error, size_t error_size);
void btech_special_object_type_register(const BtechSpecialObjectAction *action);
