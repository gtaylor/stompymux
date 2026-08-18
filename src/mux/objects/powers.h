/** @file
 * Object power definitions and power-management declarations.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"
#include "mux/support/owned_text.h"
#include "mux/world/world_context.h"

typedef struct WorldContext WorldContext;

typedef enum PowerId : int {
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

/** Executes init powertab. @param[in,out] indexes Indexes. */

extern void init_powertab(WorldIndexes *indexes);
/** Executes display powertab. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. */

extern void display_powertab(EvaluationContext *evaluation, DbRef player);
/** Sets power. @param[in,out] evaluation Expression evaluation context.
 * @param[in,out] indexes Indexes. @param[in] target Target object or value.
 * @param[in] player Player object. @param[in,out] power Power. @param[in] key
 * Lookup key or command flags. */

extern void power_set(EvaluationContext *evaluation, WorldIndexes *indexes,
                      DbRef target, DbRef player, char *power, int key);
typedef struct PowerDescriptionRequest {
  GameDatabase *database;
  DbRef viewer;
  DbRef target;
} PowerDescriptionRequest;

/** Executes power description. @param[in] request Request. */

OwnedText power_description(const PowerDescriptionRequest *request);
/** Finds find power. @param[in] indexes Indexes. @param[in] thing Thing.
 * @param[in] powername Powername. */

const POWERENT *find_power(WorldIndexes *indexes, DbRef thing,
                           const char *powername);
/** Executes decode power. @param[in,out] evaluation Expression evaluation
 * context. @param[in,out] indexes Indexes. @param[in] player Player object.
 * @param[in] powername Powername. @param[in,out] id Id. */

extern bool decode_power(EvaluationContext *evaluation, WorldIndexes *indexes,
                         DbRef player, const char *powername, PowerId *id);
typedef struct ObjectPowerRequest {
  GameDatabase *database;
  DbRef object;
  PowerId power;
} ObjectPowerRequest;

typedef struct ObjectPowerChange {
  ObjectPowerRequest target;
  bool value;
} ObjectPowerChange;

/** Executes game object has power. @param[in] request Request. */

extern bool game_object_has_power(const ObjectPowerRequest *request);
/** Sets power on game object. @param[in] change Change. */

extern void game_object_set_power(const ObjectPowerChange *change);
/** Executes game object clear powers. @param[in,out] database Game database.
 * @param[in] object Game object. */

extern void game_object_clear_powers(GameDatabase *database, DbRef object);

/** Reports whether can idle. @param[in] database Game database. @param[in] c C.
 */

static inline bool can_idle(GameDatabase *database, DbRef c) {
  return (game_object_has_power(&(ObjectPowerRequest){
              .database = database, .object = c, .power = POWER_IDLE}) ||
          is_wizard(database, c)) != 0;
}
