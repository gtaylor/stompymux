
/* Declares internal support for BattleTech special objects. */

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

typedef enum BtechSpecialStorageOperation : int {
  VERIFY = 0,
  SAVE = 1,
  LOAD = 2,
} BtechSpecialStorageOperation;

constexpr int GFLAG_ALL = 0;
constexpr int GFLAG_MECH = 1;
constexpr int GFLAG_GROUNDVEH = 2;
constexpr int GFLAG_AERO = 4;
constexpr int GFLAG_DS = 8;
constexpr int GFLAG_VTOL = 16;
constexpr int GFLAG_NAVAL = 32;
constexpr int GFLAG_BSUIT = 64;
constexpr int GFLAG_MW = 128;

#include "map.h"
#include "map_obj_api.h"
#include "special_object.h"

void btech_heartbeat_start(BtechContext *context);
void btech_heartbeat_stop(BtechContext *context);

typedef void (*BtechSpecialLifecycleHandler)(
    DbRef object, void **data, BtechSpecialLifecycleOperation operation);
typedef void (*BtechSpecialUpdateHandler)(DbRef object, void *data);
typedef size_t (*BtechSpecialStorageSize)(void);

typedef struct BtechSpecialObjectDefinition {
  const char *type;                       // Type of the object
  const BtechCommandDefinition *commands; // Commands array
  long datasize;                          // Size of private buffer
  BtechSpecialStorageSize storage_size;
  BtechSpecialLifecycleHandler lifecycle;
  int update_time;                  // Amount of time between updates (secs)
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
    SPECIAL_OBJECTS[BTECH_SPECIAL_OBJECT_COUNT];
const BtechSpecialObjectDefinition *btech_special_object_definition(int type);
size_t btech_special_command_count(int type);
const BtechCommandDefinition *btech_special_command_definition(int type,
                                                               size_t index);

void btech_registry_tree_initialize(BtechContext *context);
int btech_command_allowed_for_mech(Mech *mech, int cmdflag);
bool btech_special_command_access(BtechContext *context, DbRef object,
                                  PowerId power);
int btech_context_which_special_attribute(BtechContext *context, DbRef key);
typedef struct SpecialObjectHelpRequest {
  BtechContext *context;
  DbRef player;
  const char *type;
  int special_type;
  DbRef location;
  PowerId power_needed;
  char *argument;
} SpecialObjectHelpRequest;
void btech_special_object_help(const SpecialObjectHelpRequest *request);

void send_channel(EvaluationContext *, const char *, const char *, ...);
