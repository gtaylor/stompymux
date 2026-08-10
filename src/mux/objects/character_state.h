/* character_state.h - Typed BTech player-character state. */

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

void character_state_clear(GameDatabase *database, DbRef player);
bool character_state_exists(GameDatabase *database, DbRef player);
bool character_state_fixed_get(GameDatabase *database, DbRef player,
                               CharacterFixedState *state);
bool character_state_fixed_set(GameDatabase *database, DbRef player,
                               const CharacterFixedState *state);
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

CharacterStateEntryResult
character_state_value_entry(const CharacterStateEntryRequest *request);
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

bool character_state_value_set(const CharacterStateValueChange *change);
bool character_state_value_remove(GameDatabase *database, DbRef player,
                                  const char *name);
