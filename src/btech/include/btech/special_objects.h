/* special_objects.h - Public lifecycle operations for BTech objects. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "btech/ids.h"

typedef struct BtechContext BtechContext;

void btech_special_objects_load(BtechContext *context);
void btech_special_objects_update(BtechContext *context);
void btech_special_objects_reset(BtechContext *context);
void btech_special_object_dispose(BtechContext *context, BtechObjectId player,
                                  BtechObjectId object);
void btech_special_object_flag_changed(BtechContext *context,
                                       BtechObjectId player,
                                       BtechObjectId object,
                                       bool previously_enabled, bool enabled);
bool btech_special_object_type_can_set(BtechContext *context,
                                       BtechObjectId object, const char *type,
                                       char *error, size_t error_size);
void btech_special_object_type_register(BtechContext *context,
                                        BtechObjectId player,
                                        BtechObjectId object);
