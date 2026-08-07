/* character_state.c - Typed BTech player-character state. */

#include "mux/objects/character_state.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"

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
    free(state->values[index].name);
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

  if (!state || !(stored = state_create(database, player)))
    return false;
  stored->fixed = *state;
  return true;
}

static size_t value_find(const CharacterState *state, const char *name) {
  for (size_t index = 0; index < state->value_count; index++)
    if (strcmp(state->values[index].name, name) == 0)
      return index;
  return state->value_count;
}

size_t character_state_value_count(GameDatabase *database, DbRef player) {
  if (!valid_player(database, player) ||
      !game_database_object(database, player)->character)
    return 0;
  return game_database_object(database, player)->character->value_count;
}

bool character_state_value_entry(GameDatabase *database, DbRef player,
                                 size_t index, CharacterValueStateView *entry) {
  CharacterState *state;

  if (!valid_player(database, player) || !entry ||
      !(state = game_database_object(database, player)->character) ||
      index >= state->value_count)
    return false;
  *entry = (CharacterValueStateView){
      .name = state->values[index].name,
      .value = state->values[index].value,
      .xp = state->values[index].xp,
      .last_used = state->values[index].last_used,
  };
  return true;
}

bool character_state_value_get(GameDatabase *database, DbRef player,
                               const char *name,
                               CharacterValueStateView *entry) {
  CharacterState *state;
  size_t index;

  if (!valid_player(database, player) || !name || !entry ||
      !(state = game_database_object(database, player)->character))
    return false;
  index = value_find(state, name);
  return index < state->value_count
             ? character_state_value_entry(database, player, index, entry)
             : false;
}

bool character_state_value_set(GameDatabase *database, DbRef player,
                               const char *name, int value, int xp,
                               time_t last_used) {
  CharacterState *state;
  size_t index;

  if (!name || !*name || strlen(name) > 255 || value < 0 || value > UINT8_MAX ||
      xp < 0 || !(state = state_create(database, player)))
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
    state->values[index] = (CharacterValueState){
        .name = stored_name,
    };
    state->value_count++;
  }
  state->values[index].value = (unsigned char)value;
  state->values[index].xp = xp;
  state->values[index].last_used = last_used;
  return true;
}

bool character_state_value_remove(GameDatabase *database, DbRef player,
                                  const char *name) {
  CharacterState *state;
  size_t index;

  if (!valid_player(database, player) || !name ||
      !(state = game_database_object(database, player)->character))
    return true;
  index = value_find(state, name);
  if (index == state->value_count)
    return true;
  free(state->values[index].name);
  state->values[index] = state->values[state->value_count - 1];
  state->value_count--;
  if (state->value_count == 0) {
    free(state->values);
    state->values = nullptr;
  }
  return true;
}
