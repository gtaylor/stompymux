/* character_state.c -- Typed BTech player-character state tests. */

#include <stdlib.h>
#include <time.h>

#include "mux/objects/character_state.h"
#include "mux/objects/db.h"

bool is_good_obj(GameDatabase *database, DbRef object);

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

int main(void) {
  GameObject objects[3] = {0};
  GameDatabase database = {.object_storage = objects, .top = 2, .size = 2};
  CharacterFixedState fixed;
  CharacterValueStateView value;

  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  game_object_set_type(&database, 1, OBJECT_TYPE_THING);
  if (!character_state_fixed_get(&database, 0, &fixed) || fixed.bruise != 0 ||
      fixed.lethal != 0 || fixed.build != 1 || fixed.reflexes != 1 ||
      fixed.intuition != 1 || fixed.learn != 1 || fixed.charisma != 1 ||
      character_state_exists(&database, 0) ||
      character_state_fixed_get(&database, 1, &fixed) ||
      character_state_value_set(
          &(CharacterStateValueChange){.database = &database,
                                       .player = 1,
                                       .name = "Running",
                                       .value = 2,
                                       .experience = 3,
                                       .last_used = 4}))
    return 1;

  fixed = (CharacterFixedState){
      .bruise = 2,
      .lethal = 3,
      .build = 4,
      .reflexes = 5,
      .intuition = 6,
      .learn = 7,
      .charisma = 8,
  };
  if (!character_state_fixed_set(&database, 0, &fixed) ||
      !character_state_value_set(
          &(CharacterStateValueChange){.database = &database,
                                       .player = 0,
                                       .name = "Running",
                                       .value = 2,
                                       .experience = 300,
                                       .last_used = 123456789}) ||
      !character_state_value_set(&(CharacterStateValueChange){
          .database = &database, .player = 0, .name = "Lives"}) ||
      character_state_value_count(&database, 0) != 2 ||
      !character_state_value_get(&database, 0, "Running", &value) ||
      value.value != 2 || value.xp != 300 || value.last_used != 123456789 ||
      character_state_value_set(&(CharacterStateValueChange){
          .database = &database, .player = 0, .name = "Bad", .value = 256}) ||
      character_state_value_set(
          &(CharacterStateValueChange){.database = &database,
                                       .player = 0,
                                       .name = "Bad",
                                       .value = 1,
                                       .experience = -1}) ||
      !character_state_value_remove(&database, 0, "Lives") ||
      character_state_value_count(&database, 0) != 1)
    return 1;

  if (setenv("TZ", "America/Los_Angeles", 1) < 0)
    return 1;
  tzset();
  if (!character_state_value_get(&database, 0, "Running", &value) ||
      value.last_used != 123456789 || setenv("TZ", "Asia/Tokyo", 1) < 0)
    return 1;
  tzset();
  if (!character_state_value_get(&database, 0, "Running", &value) ||
      value.last_used != 123456789)
    return 1;

  game_object_set_type(&database, 0, OBJECT_TYPE_GARBAGE);
  character_state_clear(&database, 0);
  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  return game_database_object(&database, 0)->character ||
                 !character_state_fixed_get(&database, 0, &fixed) ||
                 fixed.build != 1 || fixed.bruise != 0
             ? 1
             : 0;
}
