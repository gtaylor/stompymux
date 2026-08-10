#pragma once

#include "mux/server/platform.h"

typedef struct BtechSpecialObject BtechSpecialObject;
typedef struct btech_object_store_context BTECH_OBJECT_STORE_CONTEXT;

void btech_store_auxiliary_object(BTECH_OBJECT_STORE_CONTEXT *context,
                                  DbRef object_id, BtechSpecialObject *object);
