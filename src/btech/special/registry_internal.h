
/*
 * $Id: glue.h,v 1.4 2005/08/08 09:43:10 murrayma Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Created: Thu Sep 19 22:02:48 1996 fingon
 * Last modified: Thu Dec 10 21:45:10 1998 fingon
 *
 */

/*
  Header for special command rooms...
  Based on the original by MUSE folks
*/

#include <stddef.h>

#include "command_catalogs.h"
#include "command_registry.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"

/* Parameter to the save/load function */
#pragma once

typedef struct EvaluationContext EvaluationContext;
typedef struct Mech Mech;
typedef struct BattleMap BattleMap;

#define VERIFY 0
#define SAVE 1
#define LOAD 2

#define XCODE_MAGIC 0x334D5442 /* v3 DB format */

#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

#define GFLAG_ALL 0
#define GFLAG_MECH 1
#define GFLAG_GROUNDVEH 2
#define GFLAG_AERO 4
#define GFLAG_DS 8
#define GFLAG_VTOL 16
#define GFLAG_NAVAL 32
#define GFLAG_BSUIT 64
#define GFLAG_MW 128

#include "map.h"
#include "map_obj_api.h"
#include "special_object.h"

void btech_heartbeat_start(BtechContext *context);
void btech_heartbeat_stop(BtechContext *context);

typedef void (*BtechSpecialLifecycleHandler)(DbRef object, void **data,
                                             int operation);
typedef void (*BtechSpecialUpdateHandler)(DbRef object, void *data);
typedef size_t (*BtechSpecialStorageSize)(void);

typedef struct BtechSpecialObjectDefinition {
  const char *type;                       // Type of the object
  const BtechCommandDefinition *commands; // Commands array
  long datasize;                          // Size of private buffer
  BtechSpecialStorageSize storage_size;
  BtechSpecialLifecycleHandler lifecycle;
  int updateTime;                   // Amount of time between updates (secs)
  BtechSpecialUpdateHandler update; // called for every object at every update
  PowerId power_needed; // What power is needed to restricted commands
} BtechSpecialObjectDefinition;

static inline size_t
btech_special_object_data_size(const BtechSpecialObjectDefinition *definition) {
  return definition->storage_size ? definition->storage_size()
                                  : (size_t)definition->datasize;
}

enum { BTECH_SPECIAL_OBJECT_COUNT = 6 };

extern const BtechSpecialObjectDefinition
    SpecialObjects[BTECH_SPECIAL_OBJECT_COUNT];

void btech_registry_tree_initialize(BtechContext *context);
int btech_command_allowed_for_mech(Mech *mech, int command_flag);
bool btech_special_command_access(BtechContext *context, DbRef object,
                                  PowerId power);
int btech_context_which_special_attribute(BtechContext *context, DbRef key);
void btech_special_object_help(BtechContext *context, DbRef player,
                               const char *type, int id, int location,
                               PowerId power_needed, int object_id,
                               char *argument);

void send_channel(EvaluationContext *, const char *, const char *, ...);
