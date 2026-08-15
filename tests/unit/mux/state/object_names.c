/* object_names.c -- Object-name cache lifetime tests. */

#include "mux/server/platform.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/objects/attrs.h"
#include "mux/objects/character_state.h"
#include "mux/objects/db.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/player_account.h"
#include "mux/objects/powers.h"
#include "mux/server/log.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/utf8.h"

size_t utf8_copy_truncated(char *destination, size_t destination_size,
                           const char *source) {
  (void)string_copy_bounded(destination, destination_size, source);
  return strlen(destination);
}

void styled_text_strip(const StyledTextPalette *palette, const char *styled,
                       char *output, size_t output_size) {
  (void)palette;
  if (output_size == 0)
    return;
  const size_t LENGTH = strlen(styled);
  if (LENGTH >= 2 &&
      *(const char *)checked_storage_at_const(styled, LENGTH + 1, sizeof(char),
                                              0) == '{' &&
      *(const char *)checked_storage_at_const(styled, LENGTH + 1, sizeof(char),
                                              LENGTH - 1) == '}') {
    const size_t COPIED =
        LENGTH - 2 < output_size - 1 ? LENGTH - 2 : output_size - 1;
    const char *content =
        checked_storage_at_const(styled, LENGTH + 1, sizeof(char), 1);
    memcpy(output, content, COPIED);
    *(char *)checked_storage_at(output, output_size, sizeof(char), COPIED) =
        '\0';
    return;
  }
  (void)string_copy_bounded(output, output_size, styled);
}

void object_state_clear(GameDatabase *database, DbRef object) {
  (void)database;
  (void)object;
}

void player_account_clear(GameDatabase *database, DbRef player) {
  (void)database;
  (void)player;
}

void character_state_clear(GameDatabase *database, DbRef player) {
  (void)database;
  (void)player;
}

void economy_parts_clear(GameDatabase *database, DbRef object) {
  (void)database;
  (void)object;
}

void log_simple(LogEntry entry, const char *message) {
  (void)entry;
  (void)message;
}

void game_object_clear_flags(GameDatabase *database, DbRef object) {
  (void)database;
  (void)object;
}

void game_object_set_flag(const ObjectFlagChangeRequest *request) {
  (void)request;
}

void game_object_clear_powers(GameDatabase *database, DbRef object) {
  (void)database;
  (void)object;
}

static int run_for_cache_setting(bool cache_names) {
  GameObject objects[3] = {};
  NAME pure_names[3] = {};
  ServerConfiguration configuration = {.cache_names = cache_names};
  GameDatabase database = {
      .object_storage = objects,
      .pure_name_storage = pure_names,
      .top = 2,
      .size = 2,
      .configuration = &configuration,
  };
  int result = 1;

  /* Run both former configurations to document that cache_names is inert. */
  objects[1].native.values[A_NAME] = strdup("{Alpha}");
  objects[2].native.values[A_NAME] = strdup("{Beta}");
  if (!objects[1].native.values[A_NAME] || !objects[2].native.values[A_NAME])
    goto cleanup;

  const char *alpha = game_object_pure_name(&database, 0);
  if (strcmp(alpha, "Alpha") != 0 ||
      alpha != game_object_pure_name(&database, 0))
    goto cleanup;

  if (strcmp(game_object_name(&database, 1), "{Beta}") != 0 ||
      strcmp(alpha, "Alpha") != 0)
    goto cleanup;

  object_name_set(&database, 0, "{Renamed}");
  if (strcmp(game_object_pure_name(&database, 0), "Renamed") != 0)
    goto cleanup;

  if (strcmp(game_object_pure_name(&database, -1), "#-1 INVALID DBREF") != 0 ||
      strcmp(game_object_pure_name(&database, database.top),
             "#-1 INVALID DBREF") != 0)
    goto cleanup;

  result = 0;
cleanup:
  free(objects[1].native.values[A_NAME]);
  free(objects[2].native.values[A_NAME]);
  free(pure_names[1]);
  free(pure_names[2]);
  return result;
}

static int check_grow_and_free(void) {
  ServerConfiguration configuration = {.init_size = 2};
  GameDatabase database;

  game_database_initialize(&database);
  database.configuration = &configuration;
  db_grow(&database, 1);
  object_name_set(&database, 0, "Persistent");
  const char *cached = game_object_pure_name(&database, 0);
  if (strcmp(cached, "Persistent") != 0)
    return 1;

  db_grow(&database, 4);
  if (cached != game_object_pure_name(&database, 0) ||
      strcmp(cached, "Persistent") != 0)
    return 1;

  db_free(&database);
  return database.object_storage != nullptr ||
         database.pure_name_storage != nullptr || database.markbits != nullptr;
}

int main(void) {
  if (run_for_cache_setting(false) != 0 || run_for_cache_setting(true) != 0 ||
      check_grow_and_free() != 0) {
    (void)fprintf(stderr, "object name cache lifetime test failed\n");
    return 1;
  }
  return 0;
}
