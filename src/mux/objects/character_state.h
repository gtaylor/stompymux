/** @file
 * Typed BTech player-character state.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <time.h>

#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;

typedef struct CharacterFixedState CharacterFixedState;
struct CharacterFixedState {
  unsigned char bruise;
  unsigned char lethal;
  unsigned char build;
  unsigned char reflexes;
  unsigned char intuition;
  unsigned char learn;
  unsigned char charisma;
};

typedef struct CharacterValueStateView CharacterValueStateView;
struct CharacterValueStateView {
  const char *name;
  unsigned char value;
  int xp;
  time_t last_used;
};

/** Clears character state. @param[in,out] database Game database. @param[in]
 * player Player object. */

void character_state_clear(GameDatabase *database, DbRef player);
/** Executes character state exists. @param[in] database Game database.
 * @param[in] player Player object. */

bool character_state_exists(GameDatabase *database, DbRef player);
/** Returns character state fixed. @param[in] database Game database. @param[in]
 * player Player object. @param[in] state State to inspect or update. */

bool character_state_fixed_get(GameDatabase *database, DbRef player,
                               CharacterFixedState *state);
/** Sets character state fixed. @param[in,out] database Game database.
 * @param[in] player Player object. @param[in] state State to inspect or update.
 */

bool character_state_fixed_set(GameDatabase *database, DbRef player,
                               const CharacterFixedState *state);
/** Counts character state value. @param[in] database Game database. @param[in]
 * player Player object. */

size_t character_state_value_count(GameDatabase *database, DbRef player);
typedef struct CharacterStateEntryRequest {
  GameDatabase *database;
  DbRef player;
  size_t index;
} CharacterStateEntryRequest;

typedef struct CharacterStateEntryResult {
  bool found;
  CharacterValueStateView entry;
} CharacterStateEntryResult;

/** Executes character state value entry. @param[in] request Request. */

CharacterStateEntryResult
character_state_value_entry(const CharacterStateEntryRequest *request);
/** Returns character state value. @param[in] database Game database. @param[in]
 * player Player object. @param[in] name Name to use. @param[in] entry Entry. */

bool character_state_value_get(GameDatabase *database, DbRef player,
                               const char *name,
                               CharacterValueStateView *entry);
typedef struct CharacterStateValueChange {
  GameDatabase *database;
  DbRef player;
  const char *name;
  int value;
  int experience;
  time_t last_used;
} CharacterStateValueChange;

/** Sets character state value. @param[in] change Change. */

bool character_state_value_set(const CharacterStateValueChange *change);
/** Removes character state value. @param[in,out] database Game database.
 * @param[in] player Player object. @param[in] name Name to use. */

bool character_state_value_remove(GameDatabase *database, DbRef player,
                                  const char *name);
