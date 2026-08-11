#pragma once

#include "mux/server/platform.h"

typedef struct BtechSpecialObject BtechSpecialObject;
typedef struct BtechObjectStoreContext BtechObjectStoreContext;

void btech_store_auxiliary_object(BtechObjectStoreContext *context,
                                  DbRef object_id, BtechSpecialObject *object);
