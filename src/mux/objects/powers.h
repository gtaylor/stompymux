/* powers.h - Object power definitions and power-management declarations. */

#pragma once

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/hash_table.h"

typedef struct WorldContext WorldContext;

typedef enum PowerId {
  POWER_NONE,
  POWER_IDLE,
  POWER_LONG_FINGERS,
  POWER_COMM_ALL,
  POWER_SEE_HIDDEN,
  POWER_NO_DESTROY,
  POWER_MECH,
  POWER_SECURITY,
  POWER_MECHREP,
  POWER_MAP,
  POWER_TEMPLATE,
  POWER_TECH,
} PowerId;

/* ---------------------------------------------------------------------------
 * POWERENT: Information about object powers.
 */

typedef struct power_entry {
  const char *powername; /* Name of the flag */
  PowerId id;
  int listperm; /* Who sees this flag when set */
} POWERENT;

typedef struct WorldIndexes WorldIndexes;
typedef struct EvaluationContext EvaluationContext;

extern void init_powertab(WorldIndexes *indexes);
extern void display_powertab(EvaluationContext *, DbRef);
extern void power_set(EvaluationContext *, WorldIndexes *, DbRef, DbRef, char *,
                      int);
extern char *power_description(GameDatabase *, DbRef, DbRef);
extern POWERENT *find_power(WorldIndexes *, DbRef, char *);
extern bool has_power(WorldContext *, DbRef, DbRef, char *);
extern bool decode_power(EvaluationContext *, WorldIndexes *, DbRef, char *,
                         PowerId *);
extern bool game_object_has_power(GameDatabase *, DbRef, PowerId);
extern void game_object_set_power(GameDatabase *, DbRef, PowerId, bool);
extern void game_object_clear_powers(GameDatabase *, DbRef);

static inline bool can_idle(GameDatabase *database, DbRef c) {
  return game_object_has_power(database, c, POWER_IDLE) ||
         is_wizard(database, c);
}
static inline bool is_long_fingers(GameDatabase *database, DbRef c) {
  return game_object_has_power(database, c, POWER_LONG_FINGERS) ||
         is_wizard(database, c);
}
static inline bool is_comm_all(GameDatabase *database, DbRef c) {
  return game_object_has_power(database, c, POWER_COMM_ALL) ||
         is_wizard(database, c);
}
/* Mecha */
static inline bool is_security(GameDatabase *database, DbRef c) {
  return game_object_has_power(database, c, POWER_SECURITY) ||
         is_wizard(database, c);
}
static inline bool is_tech_power(GameDatabase *database, DbRef c) {
  return game_object_has_power(database, c, POWER_TECH) ||
         is_wizard(database, c);
}
static inline bool is_template_power(GameDatabase *database, DbRef c) {
  return game_object_has_power(database, c, POWER_TEMPLATE) ||
         is_wizard(database, c);
}

/* End Mecha */
