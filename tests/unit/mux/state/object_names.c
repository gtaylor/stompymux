/* object_names.c -- Core object text-field lifetime tests. */

#include "mux/server/platform.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

void styled_text_strip(const StyledTextPalette *palette [[maybe_unused]],
                       const char *styled, char *output, size_t output_size) {
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

void object_state_clear(GameDatabase *database [[maybe_unused]],
                        DbRef object [[maybe_unused]]) {}

void player_account_clear(GameDatabase *database [[maybe_unused]],
                          DbRef player [[maybe_unused]]) {}

void character_state_clear(GameDatabase *database [[maybe_unused]],
                           DbRef player [[maybe_unused]]) {}

void economy_parts_clear(GameDatabase *database [[maybe_unused]],
                         DbRef object [[maybe_unused]]) {}

void log_simple(LogEntry entry [[maybe_unused]],
                const char *message [[maybe_unused]]) {}

void game_object_clear_flags(GameDatabase *database [[maybe_unused]],
                             DbRef object [[maybe_unused]]) {}

void game_object_set_flag(const ObjectFlagChangeRequest *request
                          [[maybe_unused]]) {}

void game_object_clear_powers(GameDatabase *database [[maybe_unused]],
                              DbRef object [[maybe_unused]]) {}

static int check_stable_names(void) {
  GameObject objects[3] = {};
  ServerConfiguration configuration = {};
  GameDatabase database = {
      .object_storage = objects,
      .top = 2,
      .size = 2,
      .configuration = &configuration,
  };
  int result = 1;

  object_name_set(&database, 0, "{Alpha}");
  object_name_set(&database, 1, "{Beta}");

  const char *raw_alpha = game_object_name(&database, 0);
  const char *raw_beta = game_object_name(&database, 1);
  if (strcmp(raw_alpha, "{Alpha}") != 0 || strcmp(raw_beta, "{Beta}") != 0 ||
      raw_alpha != game_object_name(&database, 0) ||
      raw_beta != game_object_name(&database, 1))
    goto cleanup;

  const char *alpha = game_object_pure_name(&database, 0);
  if (strcmp(alpha, "Alpha") != 0 ||
      alpha != game_object_pure_name(&database, 0))
    goto cleanup;

  if (strcmp(raw_alpha, "{Alpha}") != 0 || strcmp(alpha, "Alpha") != 0)
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
  game_object_owned_state_clear(&database, 0);
  game_object_owned_state_clear(&database, 1);
  return result;
}

static int check_grow_and_free(void) {
  ServerConfiguration configuration = {.init_size = 2};
  GameDatabase database;

  game_database_initialize(&database);
  database.configuration = &configuration;
  db_grow(&database, 1);
  object_name_set(&database, 0, "Persistent");
  game_object_description_set(&database, 0, "Persistent description");
  game_object_internal_description_set(&database, 0, "Persistent inside");
  const char *cached = game_object_pure_name(&database, 0);
  const char *description = game_object_description(&database, 0);
  const char *internal_description =
      game_object_internal_description(&database, 0);
  if (strcmp(cached, "Persistent") != 0)
    return 1;

  db_grow(&database, 4);
  if (cached != game_object_pure_name(&database, 0) ||
      strcmp(cached, "Persistent") != 0 ||
      description != game_object_description(&database, 0) ||
      strcmp(description, "Persistent description") != 0 ||
      internal_description != game_object_internal_description(&database, 0) ||
      strcmp(internal_description, "Persistent inside") != 0)
    return 1;

  db_free(&database);
  return database.object_storage != nullptr || database.markbits != nullptr;
}

static int check_descriptions(void) {
  GameObject objects[2] = {};
  GameDatabase database = {.object_storage = objects, .top = 1, .size = 1};
  char description[] = "description";
  char internal_description[] = "inside";

  game_object_description_set(&database, 0, description);
  game_object_internal_description_set(&database, 0, internal_description);
  description[0] = 'D';
  internal_description[0] = 'I';
  if (strcmp(game_object_description(&database, 0), "description") != 0 ||
      strcmp(game_object_internal_description(&database, 0), "inside") != 0)
    return 1;
  game_object_description_set(&database, 0, "");
  game_object_internal_description_set(&database, 0, nullptr);
  if (game_object_description(&database, 0) != nullptr ||
      game_object_internal_description(&database, 0) != nullptr)
    return 2;
  game_object_owned_state_clear(&database, 0);
  return 0;
}

int main(void) {
  if (check_stable_names() != 0 || check_grow_and_free() != 0 ||
      check_descriptions() != 0) {
    (void)fprintf(stderr, "core object text-field lifetime test failed\n");
    return 1;
  }
  return 0;
}
