#include "mux/server/database_bootstrap.h"

#include <crypto_pwhash.h>
#include <sodium/randombytes.h>
#include <sodium/utils.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/player_account.h"
#include "mux/objects/powers.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/password.h"
#include "mux/world/object.h"
#include "mux/world/object_internal.h"
#include "mux/world/player.h"

static const BootstrapObjectConfiguration *
bootstrap_object_find(const DatabaseConfiguration *configuration, DbRef dbref) {
  for (size_t index = 0; index < configuration->bootstrap_object_count;
       index++) {
    const BootstrapObjectConfiguration *object = checked_storage_at_const(
        configuration->bootstrap_objects, MAX_BOOTSTRAP_OBJECTS,
        sizeof(*configuration->bootstrap_objects), index);
    if (object->dbref == dbref)
      return object;
  }
  return nullptr;
}

static bool bootstrap_require(const DatabaseConfiguration *database,
                              DbRef dbref, const char *name,
                              BootstrapObjectType type) {
  const BootstrapObjectConfiguration *object =
      bootstrap_object_find(database, dbref);
  return object && object->type == type &&
         (name == nullptr || !strcmp(object->name, name));
}

bool database_bootstrap_god_is_wizard_player(
    const ServerConfiguration *configuration) {
  const BootstrapObjectConfiguration *god =
      bootstrap_object_find(&configuration->database, GOD);
  return god && god->type == BOOTSTRAP_OBJECT_PLAYER && god->wizard;
}

static bool bootstrap_validate(const ServerConfiguration *configuration) {
  const DatabaseConfiguration *database = &configuration->database;

  if (!bootstrap_require(database, 0, nullptr, BOOTSTRAP_OBJECT_ROOM) ||
      !bootstrap_require(database, GOD, "GOD", BOOTSTRAP_OBJECT_PLAYER) ||
      !bootstrap_require(database, 2, "Wizard", BOOTSTRAP_OBJECT_PLAYER) ||
      !bootstrap_require(database, configuration->start_room, nullptr,
                         BOOTSTRAP_OBJECT_ROOM) ||
      !bootstrap_require(database, configuration->start_home, nullptr,
                         BOOTSTRAP_OBJECT_ROOM) ||
      !bootstrap_require(database, configuration->btech_usedmechstore, nullptr,
                         BOOTSTRAP_OBJECT_ROOM) ||
      !bootstrap_require(database, configuration->afterlife_dbref, nullptr,
                         BOOTSTRAP_OBJECT_ROOM))
    return false;
  if (!database_bootstrap_god_is_wizard_player(configuration))
    return false;
  for (size_t index = 0; index < database->bootstrap_object_count; index++) {
    const BootstrapObjectConfiguration *object = checked_storage_at_const(
        database->bootstrap_objects, MAX_BOOTSTRAP_OBJECTS,
        sizeof(*database->bootstrap_objects), index);
    if (!object->name[0] || (object->type == BOOTSTRAP_OBJECT_PLAYER &&
                             object->dbref != GOD && object->dbref != 2))
      return false;
  }
  return true;
}

static void bootstrap_random_password(char output[BOOTSTRAP_PASSWORD_SIZE]) {
  unsigned char random[16];

  randombytes_buf(random, sizeof(random));
  sodium_bin2hex(output, BOOTSTRAP_PASSWORD_SIZE, random, sizeof(random));
  sodium_memzero(random, sizeof(random));
}

static const ObjectFlagSet *
bootstrap_default_flags(const ServerConfiguration *configuration,
                        BootstrapObjectType type) {
  return type == BOOTSTRAP_OBJECT_PLAYER ? &configuration->default_player_flags
                                         : &configuration->default_room_flags;
}

static const char *bootstrap_object_type_name(BootstrapObjectType type) {
  return type == BOOTSTRAP_OBJECT_PLAYER ? "player" : "room";
}

static void
bootstrap_initialize_object(EvaluationContext *evaluation,
                            const BootstrapObjectConfiguration *configuration) {
  GameDatabase *database = evaluation->world->database;
  ObjectType type = configuration->type == BOOTSTRAP_OBJECT_PLAYER
                        ? OBJECT_TYPE_PLAYER
                        : OBJECT_TYPE_ROOM;
  const ObjectFlagSet *flags = bootstrap_default_flags(
      evaluation->world->configuration, configuration->type);

  attribute_free(database, configuration->dbref);
  object_name_set(database, configuration->dbref, configuration->name);
  game_object_set_type(database, configuration->dbref, type);
  game_object_set_location(database, configuration->dbref, NOTHING);
  game_object_set_zone(database, configuration->dbref, NOTHING);
  game_object_set_contents(database, configuration->dbref, NOTHING);
  game_object_set_exits(database, configuration->dbref, NOTHING);
  game_object_set_link(database, configuration->dbref, NOTHING);
  game_object_set_next(database, configuration->dbref, NOTHING);
  game_object_clear_flags(database, configuration->dbref);
  for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++) {
    game_object_set_flag(
        &(ObjectFlagChangeRequest){.database = database,
                                   .object = configuration->dbref,
                                   .flag = flag,
                                   .value = object_flag_set_has(flags, flag)});
  }
  game_object_clear_powers(database, configuration->dbref);
  object_apply_default_lua_parent(
      &(ObjectCreationIdentity){.evaluation = evaluation,
                                .object = configuration->dbref,
                                .type = (int)type});
}

static bool
bootstrap_initialize_player(EvaluationContext *evaluation,
                            const BootstrapObjectConfiguration *player,
                            const char *password) {
  GameDatabase *database = evaluation->world->database;
  ServerConfiguration *configuration = evaluation->world->configuration;
  char hash[crypto_pwhash_STRBYTES];
  time_t now = time(nullptr);

  if (!password_hash(configuration, password, hash) ||
      !player_account_password_hash_set(database, player->dbref, hash) ||
      !player_account_last_login_set(&(PlayerLastLoginChange){
          .account = {.database = database, .player = player->dbref},
          .occurred_at = now == (time_t)-1 ? 0 : now})) {
    sodium_memzero(hash, sizeof(hash));
    return false;
  }
  sodium_memzero(hash, sizeof(hash));
  game_object_set_flag(&(ObjectFlagChangeRequest){.database = database,
                                                  .object = player->dbref,
                                                  .flag = OBJECT_FLAG_WIZARD,
                                                  .value = player->wizard});
  game_object_set_location(database, player->dbref, configuration->start_room);
  game_object_set_link(database, player->dbref, configuration->start_home);
  game_object_set_next(
      database, player->dbref,
      game_object_contents(database, configuration->start_room));
  game_object_set_contents(database, configuration->start_room, player->dbref);
  return true;
}

int database_bootstrap(EvaluationContext *evaluation,
                       char god_password[BOOTSTRAP_PASSWORD_SIZE],
                       char wizard_password[BOOTSTRAP_PASSWORD_SIZE]) {
  ServerConfiguration *configuration = evaluation->world->configuration;
  DatabaseConfiguration *database_configuration = &configuration->database;
  DbRef top = 0;

  if (!bootstrap_validate(configuration)) {
    (void)fprintf(stderr, "Invalid database bootstrap object configuration.\n");
    return -1;
  }
  log_error((LogEntry){.log = evaluation->log,
                       .key = LOG_ALWAYS,
                       .primary = "INI",
                       .secondary = "BOOT"},
            "Bootstrapping database: %s", database_configuration->gamedb);
  for (size_t index = 0; index < database_configuration->bootstrap_object_count;
       index++) {
    const BootstrapObjectConfiguration *object = checked_storage_at_const(
        database_configuration->bootstrap_objects, MAX_BOOTSTRAP_OBJECTS,
        sizeof(*database_configuration->bootstrap_objects), index);
    if (object->dbref >= top)
      top = object->dbref + 1;
  }
  db_free(evaluation->world->database);
  db_grow(evaluation->world->database, top);
  for (size_t index = 0; index < database_configuration->bootstrap_object_count;
       index++) {
    const BootstrapObjectConfiguration *object = checked_storage_at_const(
        database_configuration->bootstrap_objects, MAX_BOOTSTRAP_OBJECTS,
        sizeof(*database_configuration->bootstrap_objects), index);
    bootstrap_initialize_object(evaluation, object);
    log_error((LogEntry){.log = evaluation->log,
                         .key = LOG_ALWAYS,
                         .primary = "INI",
                         .secondary = "BOOT"},
              "Created bootstrap object: %s (#%ld), type %s", object->name,
              object->dbref, bootstrap_object_type_name(object->type));
  }

  bootstrap_random_password(god_password);
  bootstrap_random_password(wizard_password);
#ifdef BTECH_PERSISTENCE_TESTING
  const char *test_password = getenv("BTECH_TEST_GOD_PASSWORD");
  if (test_password && *test_password)
    (void)snprintf(god_password, BOOTSTRAP_PASSWORD_SIZE, "%s", test_password);
#endif
  if (!bootstrap_initialize_player(
          evaluation, bootstrap_object_find(database_configuration, GOD),
          god_password) ||
      !bootstrap_initialize_player(
          evaluation, bootstrap_object_find(database_configuration, 2),
          wizard_password))
    return -1;
  load_player_names(evaluation->world);
  object_make_freelist(evaluation->world->database);
  return 0;
}
