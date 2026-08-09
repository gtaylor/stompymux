/* character_state_adapter.c -- PSTATS/typed character-state adapter tests. */

#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <time.h>

#include "btech/context.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_global.h"
#include "btechstats_internal.h"
#include "mux/objects/character_state.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

struct CharacterValue char_values[NUM_CHARVALUES];

static size_t stats_index(int code) {
  if (code < 0 || code >= NUM_CHARVALUES)
    abort();
  return (size_t)code;
}

const CharacterValue *character_value_definition(int code) {
  return checked_storage_at_const(char_values, NUM_CHARVALUES,
                                  sizeof(*char_values), stats_index(code));
}

static CharacterValue *character_value_mutable(int code) {
  return checked_storage_at(char_values, NUM_CHARVALUES, sizeof(*char_values),
                            stats_index(code));
}

unsigned char character_stats_value_get(const PSTATS *stats, int code) {
  return *(const unsigned char *)checked_storage_at_const(
      stats->value_storage, NUM_CHARVALUES, sizeof(*stats->value_storage),
      stats_index(code));
}

void character_stats_value_set(PSTATS *stats, int code, int value) {
  *(unsigned char *)checked_storage_at(
      stats->value_storage, NUM_CHARVALUES, sizeof(*stats->value_storage),
      stats_index(code)) = (unsigned char)value;
}

int character_stats_xp_get(const PSTATS *stats, int code) {
  return *(const int *)checked_storage_at_const(
      stats->xp_storage, NUM_CHARVALUES, sizeof(*stats->xp_storage),
      stats_index(code));
}

void character_stats_xp_set(PSTATS *stats, int code, int value) {
  *(int *)checked_storage_at(stats->xp_storage, NUM_CHARVALUES,
                             sizeof(*stats->xp_storage), stats_index(code)) =
      value;
}

time_t character_stats_last_use_get(const PSTATS *stats, int code) {
  return *(const time_t *)checked_storage_at_const(
      stats->last_use_storage, NUM_CHARVALUES, sizeof(*stats->last_use_storage),
      stats_index(code));
}

void character_stats_last_use_set(PSTATS *stats, int code, time_t value) {
  *(time_t *)checked_storage_at(stats->last_use_storage, NUM_CHARVALUES,
                                sizeof(*stats->last_use_storage),
                                stats_index(code)) = value;
}

bool is_good_obj(GameDatabase *database, DbRef object);

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

int char_getvaluecode(BtechContext *context, const char *name) {
  (void)context;
  for (int code = 0; code < NUM_CHARVALUES; code++)
    if (character_value_definition(code)->name &&
        strcasecmp(character_value_definition(code)->name, name) == 0)
      return code;
  return -1;
}

void char_setstatvalue(PSTATS *stats, const char *name, int value) {
  int code = char_getvaluecode(nullptr, name);
  if (code >= 0)
    character_stats_value_set(stats, code, value);
}

static void initialize_catalog(void) {
  *character_value_mutable(LIVES_NUMBER) =
      (struct CharacterValue){.name = "Lives", .type = CHAR_VALUE};
  *character_value_mutable(6) =
      (struct CharacterValue){.name = "Bruise", .type = CHAR_VALUE};
  *character_value_mutable(7) =
      (struct CharacterValue){.name = "Lethal", .type = CHAR_VALUE};
  *character_value_mutable(26) =
      (struct CharacterValue){.name = "Toughness", .type = CHAR_ADVANTAGE};
  *character_value_mutable(37) =
      (struct CharacterValue){.name = "Build", .type = CHAR_ATTRIBUTE};
  *character_value_mutable(38) =
      (struct CharacterValue){.name = "Reflexes", .type = CHAR_ATTRIBUTE};
  *character_value_mutable(39) =
      (struct CharacterValue){.name = "Intuition", .type = CHAR_ATTRIBUTE};
  *character_value_mutable(40) =
      (struct CharacterValue){.name = "Learn", .type = CHAR_ATTRIBUTE};
  *character_value_mutable(41) =
      (struct CharacterValue){.name = "Charisma", .type = CHAR_ATTRIBUTE};
  *character_value_mutable(99) =
      (struct CharacterValue){.name = "Running", .type = CHAR_SKILL};
}

int main(void) {
  GameObject objects[3] = {0};
  GameDatabase database = {.object_storage = objects, .top = 2, .size = 2};
  BtechContext context = {
      .database = &database,
      .cached_target_character = 0,
  };
  PSTATS stats;
  PSTATS update = {0};

  initialize_catalog();
  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  game_object_set_type(&database, 1, OBJECT_TYPE_THING);

  character_stats_retrieve(&context, 0, VALUES_ALL, &stats);
  if (character_stats_value_get(&stats, LIVES_NUMBER) != 1 ||
      character_stats_value_get(&stats, 6) != 0 ||
      character_stats_value_get(&stats, 7) != 0 ||
      character_stats_value_get(&stats, 37) != 1 ||
      character_stats_value_get(&stats, 38) != 1 ||
      character_stats_value_get(&stats, 39) != 1 ||
      character_stats_value_get(&stats, 40) != 1 ||
      character_stats_value_get(&stats, 41) != 1)
    return 1;

  character_stats_value_set(&update, 37, 4);
  character_stats_value_set(&update, 38, 5);
  character_stats_value_set(&update, 39, 6);
  character_stats_value_set(&update, 40, 7);
  character_stats_value_set(&update, 41, 8);
  character_stats_store(&context, 0, &update, VALUES_ATTRS);

  memset(&update, 0, sizeof(update));
  character_stats_value_set(&update, 6, 2);
  character_stats_value_set(&update, 7, 3);
  character_stats_store(&context, 0, &update, VALUES_HEALTH);

  memset(&update, 0, sizeof(update));
  character_stats_value_set(&update, 99, 2);
  character_stats_xp_set(&update, 99, 300);
  character_stats_last_use_set(&update, 99, 123456789);
  character_stats_store(&context, 0, &update, VALUES_SKILLS);

  memset(&update, 0, sizeof(update));
  character_stats_value_set(&update, LIVES_NUMBER, 0);
  character_stats_value_set(&update, 26, 1);
  character_stats_store(&context, 0, &update, VALUES_ADVS);

  character_stats_retrieve(&context, 0, VALUES_ALL, &stats);
  if (character_stats_value_get(&stats, 6) != 2 ||
      character_stats_value_get(&stats, 7) != 3 ||
      character_stats_value_get(&stats, 37) != 4 ||
      character_stats_value_get(&stats, 38) != 5 ||
      character_stats_value_get(&stats, 39) != 6 ||
      character_stats_value_get(&stats, 40) != 7 ||
      character_stats_value_get(&stats, 41) != 8 ||
      character_stats_value_get(&stats, 99) != 2 ||
      character_stats_xp_get(&stats, 99) != 300 ||
      character_stats_last_use_get(&stats, 99) != 123456789 ||
      character_stats_value_get(&stats, LIVES_NUMBER) != 0 ||
      character_stats_value_get(&stats, 26) != 1 ||
      context.cached_target_character != -1 ||
      !character_state_validate_all(&context))
    return 1;

  if (!character_state_value_set(&database, 0, "running", 1, 0, 0) ||
      character_state_validate_all(&context) ||
      !character_state_value_remove(&database, 0, "running") ||
      !character_state_validate_all(&context))
    return 1;

  character_stats_value_set(&stats, 99, 4);
  character_stats_xp_set(&stats, 99, 500);
  character_stats_last_use_set(&stats, 99, 987654321);
  character_stats_clear(&stats);
  if (character_stats_value_get(&stats, 99) != 0 ||
      character_stats_xp_get(&stats, 99) != 0 ||
      character_stats_last_use_get(&stats, 99) != 0 ||
      character_stats_value_get(&stats, 37) != 1 ||
      character_stats_value_get(&stats, LIVES_NUMBER) != 1)
    return 1;

  character_stats_retrieve(&context, 1, VALUES_ALL, &stats);
  character_state_clear(&database, 0);
  return character_state_exists(&database, 0) ? 1 : 0;
}
