/* character_state_adapter.c -- PSTATS/typed character-state adapter tests. */

#include <string.h>

#include "btechstats_internal.h"

struct CharacterValue char_values[NUM_CHARVALUES];

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

int char_getvaluecode(BtechContext *context, const char *name) {
  (void)context;
  for (int code = 0; code < NUM_CHARVALUES; code++)
    if (char_values[code].name && strcasecmp(char_values[code].name, name) == 0)
      return code;
  return -1;
}

void char_setstatvalue(PSTATS *stats, char *name, int value) {
  int code = char_getvaluecode(nullptr, name);
  if (code >= 0)
    stats->values[code] = (unsigned char)value;
}

static void initialize_catalog(void) {
  char_values[LIVES_NUMBER] =
      (struct CharacterValue){.name = "Lives", .type = CHAR_VALUE};
  char_values[6] =
      (struct CharacterValue){.name = "Bruise", .type = CHAR_VALUE};
  char_values[7] =
      (struct CharacterValue){.name = "Lethal", .type = CHAR_VALUE};
  char_values[26] =
      (struct CharacterValue){.name = "Toughness", .type = CHAR_ADVANTAGE};
  char_values[37] =
      (struct CharacterValue){.name = "Build", .type = CHAR_ATTRIBUTE};
  char_values[38] =
      (struct CharacterValue){.name = "Reflexes", .type = CHAR_ATTRIBUTE};
  char_values[39] =
      (struct CharacterValue){.name = "Intuition", .type = CHAR_ATTRIBUTE};
  char_values[40] =
      (struct CharacterValue){.name = "Learn", .type = CHAR_ATTRIBUTE};
  char_values[41] =
      (struct CharacterValue){.name = "Charisma", .type = CHAR_ATTRIBUTE};
  char_values[99] =
      (struct CharacterValue){.name = "Running", .type = CHAR_SKILL};
}

int main(void) {
  GameObject objects[2] = {0};
  GameDatabase database = {.objects = objects, .top = 2, .size = 2};
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
  if (stats.values[LIVES_NUMBER] != 1 || stats.values[6] != 0 ||
      stats.values[7] != 0 || stats.values[37] != 1 ||
      stats.values[38] != 1 || stats.values[39] != 1 ||
      stats.values[40] != 1 || stats.values[41] != 1)
    return 1;

  update.values[37] = 4;
  update.values[38] = 5;
  update.values[39] = 6;
  update.values[40] = 7;
  update.values[41] = 8;
  character_stats_store(&context, 0, &update, VALUES_ATTRS);

  memset(&update, 0, sizeof(update));
  update.values[6] = 2;
  update.values[7] = 3;
  character_stats_store(&context, 0, &update, VALUES_HEALTH);

  memset(&update, 0, sizeof(update));
  update.values[99] = 2;
  update.xp[99] = 300;
  update.last_use[99] = 123456789;
  character_stats_store(&context, 0, &update, VALUES_SKILLS);

  memset(&update, 0, sizeof(update));
  update.values[LIVES_NUMBER] = 0;
  update.values[26] = 1;
  character_stats_store(&context, 0, &update, VALUES_ADVS);

  character_stats_retrieve(&context, 0, VALUES_ALL, &stats);
  if (stats.values[6] != 2 || stats.values[7] != 3 ||
      stats.values[37] != 4 || stats.values[38] != 5 ||
      stats.values[39] != 6 || stats.values[40] != 7 ||
      stats.values[41] != 8 || stats.values[99] != 2 ||
      stats.xp[99] != 300 || stats.last_use[99] != 123456789 ||
      stats.values[LIVES_NUMBER] != 0 || stats.values[26] != 1 ||
      context.cached_target_character != -1 ||
      !character_state_validate_all(&context))
    return 1;

  if (!character_state_value_set(&database, 0, "running", 1, 0, 0) ||
      character_state_validate_all(&context) ||
      !character_state_value_remove(&database, 0, "running") ||
      !character_state_validate_all(&context))
    return 1;

  stats.values[99] = 4;
  stats.xp[99] = 500;
  stats.last_use[99] = 987654321;
  character_stats_clear(&stats);
  if (stats.values[99] != 0 || stats.xp[99] != 0 ||
      stats.last_use[99] != 0 || stats.values[37] != 1 ||
      stats.values[LIVES_NUMBER] != 1)
    return 1;

  character_stats_retrieve(&context, 1, VALUES_ALL, &stats);
  character_state_clear(&database, 0);
  return character_state_exists(&database, 0) ? 1 : 0;
}
