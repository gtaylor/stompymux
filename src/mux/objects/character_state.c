/* character_state.c - Typed BTech player-character state. */

#include "mux/objects/character_state.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

typedef struct CharacterValueState CharacterValueState;
struct CharacterValueState {
  char *name;
  unsigned char value;
  int xp;
  time_t last_used;
};

struct CharacterState {
  CharacterFixedState fixed;
  CharacterValueState *values;
  size_t value_count;
};

static CharacterValueState *character_value(CharacterValueState *values,
                                            size_t count, size_t index) {
  return checked_storage_at(values, count, sizeof(*values), index);
}

static const CharacterFixedState default_fixed_state = {
    .build = 1,
    .reflexes = 1,
    .intuition = 1,
    .learn = 1,
    .charisma = 1,
};

static bool valid_player(GameDatabase *database, DbRef player) {
  return is_good_obj(database, player) &&
         game_object_type(database, player) == OBJECT_TYPE_PLAYER;
}

static CharacterState *state_create(GameDatabase *database, DbRef player) {
  CharacterState *state;

  if (!valid_player(database, player))
    return nullptr;
  state = game_database_object(database, player)->character;
  if (state)
    return state;
  state = calloc(1, sizeof(*state));
  if (!state)
    return nullptr;
  state->fixed = default_fixed_state;
  game_database_object(database, player)->character = state;
  return state;
}

void character_state_clear(GameDatabase *database, DbRef player) {
  CharacterState *state;

  if (!database || player < 0 || player >= database->top)
    return;
  state = game_database_object(database, player)->character;
  if (!state)
    return;
  for (size_t index = 0; index < state->value_count; index++)
    free(character_value(state->values, state->value_count, index)->name);
  free(state->values);
  free(state);
  game_database_object(database, player)->character = nullptr;
}

bool character_state_exists(GameDatabase *database, DbRef player) {
  return valid_player(database, player) &&
         game_database_object(database, player)->character != nullptr;
}

bool character_state_fixed_get(GameDatabase *database, DbRef player,
                               CharacterFixedState *state) {
  CharacterState *stored;

  if (!valid_player(database, player) || !state)
    return false;
  stored = game_database_object(database, player)->character;
  *state = stored ? stored->fixed : default_fixed_state;
  return true;
}

bool character_state_fixed_set(GameDatabase *database, DbRef player,
                               const CharacterFixedState *state) {
  CharacterState *stored;

  if (!state)
    return false;
  stored = state_create(database, player);
  if (!stored)
    return false;
  stored->fixed = *state;
  return true;
}

static size_t value_find(const CharacterState *state, const char *name) {
  for (size_t index = 0; index < state->value_count; index++)
    if (strcmp(character_value(state->values, state->value_count, index)->name,
               name) == 0)
      return index;
  return state->value_count;
}

size_t character_state_value_count(GameDatabase *database, DbRef player) {
  if (!valid_player(database, player) ||
      !game_database_object(database, player)->character)
    return 0;
  return game_database_object(database, player)->character->value_count;
}

CharacterStateEntryResult
character_state_value_entry(const CharacterStateEntryRequest *request) {
  GameDatabase *database = request->database;
  DbRef player = request->player;
  size_t index = request->index;
  CharacterState *state;

  if (!valid_player(database, player))
    return (CharacterStateEntryResult){0};
  state = game_database_object(database, player)->character;
  if (!state || index >= state->value_count)
    return (CharacterStateEntryResult){0};
  const CharacterValueState *stored =
      character_value(state->values, state->value_count, index);
  return (CharacterStateEntryResult){.found = true,
                                     .entry = {.name = stored->name,
                                               .value = stored->value,
                                               .xp = stored->xp,
                                               .last_used = stored->last_used}};
}

bool character_state_value_get(GameDatabase *database, DbRef player,
                               const char *name,
                               CharacterValueStateView *entry) {
  CharacterState *state;
  size_t index;

  if (!valid_player(database, player) || !name || !entry)
    return false;
  state = game_database_object(database, player)->character;
  if (!state)
    return false;
  index = value_find(state, name);
  if (index >= state->value_count)
    return false;
  CharacterStateEntryResult result =
      character_state_value_entry(&(CharacterStateEntryRequest){
          .database = database, .player = player, .index = index});
  if (result.found)
    *entry = result.entry;
  return result.found;
}

bool character_state_value_set(const CharacterStateValueChange *change) {
  GameDatabase *database = change->database;
  DbRef player = change->player;
  const char *name = change->name;
  int value = change->value;
  int xp = change->experience;
  time_t last_used = change->last_used;
  CharacterState *state;
  size_t index;

  if (!name || !*name || strlen(name) > 255 || value < 0 || value > UINT8_MAX ||
      xp < 0)
    return false;
  state = state_create(database, player);
  if (!state)
    return false;
  index = value_find(state, name);
  if (index == state->value_count) {
    if (state->value_count == SIZE_MAX / sizeof(*state->values))
      return false;
    char *stored_name = strdup(name);
    if (!stored_name)
      return false;
    CharacterValueState *grown = realloc(
        state->values, (state->value_count + 1) * sizeof(*state->values));
    if (!grown) {
      free(stored_name);
      return false;
    }
    state->values = grown;
    *character_value(state->values, state->value_count + 1, index) =
        (CharacterValueState){
            .name = stored_name,
        };
    state->value_count++;
  }
  CharacterValueState *stored =
      character_value(state->values, state->value_count, index);
  stored->value = (unsigned char)value;
  stored->xp = xp;
  stored->last_used = last_used;
  return true;
}

bool character_state_value_remove(GameDatabase *database, DbRef player,
                                  const char *name) {
  CharacterState *state;
  size_t index;

  if (!valid_player(database, player) || !name)
    return true;
  state = game_database_object(database, player)->character;
  if (!state)
    return true;
  index = value_find(state, name);
  if (index == state->value_count)
    return true;
  free(character_value(state->values, state->value_count, index)->name);
  *character_value(state->values, state->value_count, index) = *character_value(
      state->values, state->value_count, state->value_count - 1);
  state->value_count--;
  if (state->value_count == 0) {
    free(state->values);
    state->values = nullptr;
  }
  return true;
}
