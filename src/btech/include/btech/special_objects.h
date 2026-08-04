/* special_objects.h - Public lifecycle operations for BTech objects. */

#pragma once

#include <stdbool.h>

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
