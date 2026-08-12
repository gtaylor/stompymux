/* powers.h - Object power definitions and power-management declarations. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"
#include "mux/world/world_context.h"

typedef struct WorldContext WorldContext;

typedef enum PowerId {
  POWER_NONE,
  POWER_IDLE,
  POWER_COUNT,
} PowerId;

/* ---------------------------------------------------------------------------
 * POWERENT: Information about object powers.
 */

typedef struct PowerEntry {
  const char *powername; /* Name of the flag */
  PowerId id;
  int listperm; /* Who sees this flag when set */
} POWERENT;

typedef struct WorldIndexes WorldIndexes;
typedef struct EvaluationContext EvaluationContext;

extern void init_powertab(WorldIndexes *indexes);
extern void display_powertab(EvaluationContext * /*evaluation*/,
                             DbRef /*player*/);
extern void power_set(EvaluationContext * /*evaluation*/,
                      WorldIndexes * /*indexes*/, DbRef /*target*/,
                      DbRef /*player*/, char * /*power*/, int /*key*/);
typedef struct PowerDescriptionRequest {
  GameDatabase *database;
  DbRef viewer;
  DbRef target;
} PowerDescriptionRequest;

extern char *power_description(const PowerDescriptionRequest *request);
extern POWERENT *find_power(WorldIndexes * /*indexes*/, DbRef /*thing*/,
                            char * /*powername*/);
extern bool decode_power(EvaluationContext * /*evaluation*/,
                         WorldIndexes * /*indexes*/, DbRef /*player*/,
                         char * /*powername*/, PowerId * /*id*/);
typedef struct ObjectPowerRequest {
  GameDatabase *database;
  DbRef object;
  PowerId power;
} ObjectPowerRequest;

typedef struct ObjectPowerChange {
  ObjectPowerRequest target;
  bool value;
} ObjectPowerChange;

extern bool game_object_has_power(const ObjectPowerRequest *request);
extern void game_object_set_power(const ObjectPowerChange *change);
extern void game_object_clear_powers(GameDatabase * /*database*/,
                                     DbRef /*object*/);

static inline bool can_idle(GameDatabase *database, DbRef c) {
  return game_object_has_power(&(ObjectPowerRequest){
             .database = database, .object = c, .power = POWER_IDLE}) ||
         is_wizard(database, c);
}
