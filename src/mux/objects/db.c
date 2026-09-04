/* db.c - In-memory game-object and attribute database operations. */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/commands/macro.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/objects/character_state.h"
#include "mux/objects/db.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/player_account.h"
#include "mux/objects/powers.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/utf8.h"
#include "mux/support/validation.h"
#include "mux/world/object.h"
#include "mux/world/player.h"

/*
 * Restart definitions
 */

void game_database_initialize(GameDatabase *database) {
  memset(database, 0, sizeof(*database));
  database->freelist = NOTHING;
}

void game_database_bind_services(GameDatabase *database,
                                 ServerConfiguration *configuration,
                                 WorldIndexes *indexes,
                                 DescriptorRegistry *descriptors,
                                 PlayerCache *players, ServerLog *log,
                                 StyledTextPalette *palette) {
  database->configuration = configuration;
  database->indexes = indexes;
  database->descriptors = descriptors;
  database->players = players;
  database->log = log;
  database->styled_text_palette = palette;
}

void game_database_destroy(GameDatabase *database) {
  if (database == nullptr)
    return;
  db_free(database);
}

static char *set_string(char **ptr, const char *new) {
  char *copy = nullptr;

  if (new) {
    copy = checked_storage_allocate(strlen(new) + 1);
    (void)string_copy_bounded(copy, strlen(new) + 1, new);
  }
  free(*ptr);
  *ptr = copy;
  return copy;
}

/*
 * ---------------------------------------------------------------------------
 * * Name, s_Name: Get or set an object's name.
 */
const char *game_object_name(GameDatabase *database, DbRef thing) {
  if (thing >= database->top || thing < 0) {
    return "#-1 INVALID DBREF";
  }
  GameObject *object = game_database_object(database, thing);
  return object->name ? object->name : "";
}

const char *game_object_lua_parent(GameDatabase *database, DbRef object) {
  const char *path = game_database_object(database, object)->lua_parent;

  return path ? path : "";
}

bool game_object_lua_parent_set(GameDatabase *database, DbRef object,
                                const char *path) {
  char *copy = nullptr;

  if (path && *path) {
    copy = strdup(path);
    if (!copy)
      return false;
  }
  free(game_database_object(database, object)->lua_parent);
  game_database_object(database, object)->lua_parent = copy;
  return true;
}

const char *game_object_pure_name(GameDatabase *database, DbRef thing) {
  if (thing >= database->top || thing < 0) {
    return "#-1 INVALID DBREF";
  }
  GameObject *object = game_database_object(database, thing);
  return object->pure_name ? object->pure_name : "";
}

void object_name_set(GameDatabase *database, DbRef thing, const char *s) {
  char stored[MBUF_SIZE];
  char new[MBUF_SIZE];

  utf8_copy_truncated(stored, sizeof(stored), s);
  GameObject *object = game_database_object(database, thing);
  set_string(&object->name, stored);
  styled_text_strip(database->styled_text_palette, stored, new, sizeof(new));
  set_string(&object->pure_name, new);
}

const char *game_object_description(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->description;
}

void game_object_description_set(GameDatabase *database, DbRef object,
                                 const char *description) {
  set_string(&game_database_object(database, object)->description,
             description && *description ? description : nullptr);
}

const char *game_object_internal_description(GameDatabase *database,
                                             DbRef object) {
  return game_database_object(database, object)->internal_description;
}

void game_object_internal_description_set(GameDatabase *database, DbRef object,
                                          const char *description) {
  set_string(&game_database_object(database, object)->internal_description,
             description && *description ? description : nullptr);
}

bool object_password_set(GameDatabase *database, DbRef thing, const char *s) {
  return player_account_password_hash_set(database, thing, s);
}

void game_object_owned_state_clear(GameDatabase *database, DbRef thing) {
  GameObject *game_object = game_database_object(database, thing);
  free(game_object->name);
  game_object->name = nullptr;
  free(game_object->pure_name);
  game_object->pure_name = nullptr;
  free(game_object->description);
  game_object->description = nullptr;
  free(game_object->internal_description);
  game_object->internal_description = nullptr;
  free(game_object->lua_parent);
  game_object->lua_parent = nullptr;
  game_object->pending_destroyer = NOTHING;
  object_state_clear(database, thing);
  player_account_clear(database, thing);
  character_state_clear(database, thing);
  economy_parts_clear(database, thing);
}

/*
 * ---------------------------------------------------------------------------
 * * game_object_owned_state_copy: Copy core-owned state between objects. Takes
 * the
 * * player argument to ensure that only attributes that COULD be set by
 * * the player are copied.
 */

void game_object_owned_state_copy(
    const GameObjectOwnedStateCopyRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef dest = request->destination;
  DbRef source = request->source;
  GameObject *source_object =
      game_database_object(evaluation->world->database, source);
  object_state_copy(evaluation->world->database, dest, source);
  game_object_lua_parent_set(evaluation->world->database, dest,
                             source_object->lua_parent);
  game_object_description_set(evaluation->world->database, dest,
                              source_object->description);
  game_object_internal_description_set(evaluation->world->database, dest,
                                       source_object->internal_description);
}

/*
 * ---------------------------------------------------------------------------
 * * db_grow: Extend the struct database.
 */

// So mistaken refs to #-1 won't die.
static constexpr int SIZE_HACK = 1;

static void initialize_objects(GameDatabase *database, DbRef first,
                               DbRef last) {
  DbRef thing;

  for (thing = first; thing < last; thing++) {
    memset(game_database_object(database, thing), 0, sizeof(GameObject));
    game_object_renew_generation(database, thing);
    game_object_set_type(database, thing, OBJECT_TYPE_GARBAGE);
    game_object_clear_flags(database, thing);
    s_going(database, thing);
    game_object_clear_powers(database, thing);
    game_object_set_location(database, thing, NOTHING);
    game_object_set_contents(database, thing, NOTHING);
    game_object_set_exits(database, thing, NOTHING);
    game_object_set_link(database, thing, NOTHING);
    game_object_set_next(database, thing, NOTHING);
    game_object_set_zone(database, thing, NOTHING);
    game_object_set_affiliation(database, thing, NOTHING);
    game_database_object(database, thing)->pending_destroyer = NOTHING;
    game_object_set_stack(database, thing, nullptr);
    game_database_object(database, thing)->state = nullptr;
  }
}

void db_grow(GameDatabase *database, DbRef newtop) {
  char message_buffer[128];
  int newsize;
  int marksize;
  int delta;
  int i;
  DatabaseMarkBuffer *newmarkbuf;
  GameObject *newdb;

  char *cp;

  delta = database->configuration->init_size;

  /*
   * Determine what to do based on requested size, current top and  * *
   *
   * *  * *  * *  * * size.  Make sure we grow in reasonable-sized
   * chunks to * * prevent *  * *  * frequent reallocations of the
   * database->objects array.
   */

  /*
   * If requested size is smaller than the current database->objects size,
   * ignore it
   */

  if (newtop <= database->top) {
    return;
  }
  /*
   * If requested size is greater than the current database->objects size but
   * smaller
   * * * * * * * than the amount of space we have allocated, raise the
   * database->objects  * *  * size * * and * initialize the new area.
   */

  if (newtop <= database->size) {
    initialize_objects(database, database->top, newtop);
    database->top = (int)newtop;
    return;
  }
  /*
   * Grow by a minimum of delta objects
   */

  if (newtop <= database->size + delta) {
    newsize = database->size + delta;
  } else {
    newsize = (int)newtop;
  }

  /*
   * Enforce minimumdatabase size
   */

  if (newsize < database->minimum_size)
    newsize = database->minimum_size + delta;

  /*
   * Grow the database->objects array
   */

  newdb = newsize < 0 ? nullptr
                      : (GameObject *)checked_storage_try_allocate_array(
                            (size_t)newsize + SIZE_HACK, sizeof(GameObject));
  if (!newdb) {

    (void)snprintf(message_buffer, sizeof(message_buffer),
                   "Could not allocate space for %d item struct database.",
                   newsize);
    log_simple((LogEntry){.log = database->log,
                          .key = LOG_ALWAYS,
                          .primary = "ALC",
                          .secondary = "DB"},
               message_buffer);
    abort();
  }
  database->size = newsize;
  if (database->object_storage) {

    /*
     * An old struct database exists.  Copy it to the new buffer
     */

    memmove(newdb, database->object_storage,
            (size_t)(database->top + SIZE_HACK) * sizeof(GameObject));
    cp = (char *)database->object_storage;
    free(cp);
  } else {

    /*
     * Creating a brand new struct database.  Fill in the * * * *
     *
     * *  * * 'reserved' area in case it gets referenced.
     */

    database->object_storage = newdb;
    for (i = 0; i < SIZE_HACK; i++) {
      const DbRef RESERVED = i - SIZE_HACK;
      game_object_set_type(database, RESERVED, OBJECT_TYPE_GARBAGE);
      game_object_clear_flags(database, RESERVED);
      s_going(database, RESERVED);
      game_object_clear_powers(database, RESERVED);
      game_object_set_location(database, RESERVED, NOTHING);
      game_object_set_contents(database, RESERVED, NOTHING);
      game_object_set_exits(database, RESERVED, NOTHING);
      game_object_set_link(database, RESERVED, NOTHING);
      game_object_set_next(database, RESERVED, NOTHING);
      game_object_set_zone(database, RESERVED, NOTHING);
      game_object_set_affiliation(database, RESERVED, NOTHING);
      game_database_object(database, RESERVED)->pending_destroyer = NOTHING;
      game_object_set_stack(database, RESERVED, nullptr);
      game_database_object(database, RESERVED)->state = nullptr;
    }
  }
  database->object_storage = newdb;
  newdb = nullptr;

  initialize_objects(database, database->top, newtop);
  database->top = (int)newtop;

  /*
   * Grow the database->objects mark buffer
   */

  marksize = (newsize + 7) >> 3;
  newmarkbuf = (DatabaseMarkBuffer *)checked_storage_allocate((size_t)marksize);
  memset(newmarkbuf, 0, (size_t)marksize);
  if (database->markbits) {
    marksize = (int)((newtop + 7) >> 3);
    memmove(newmarkbuf, database->markbits, (size_t)marksize);
    cp = (char *)database->markbits;
    free(cp);
  }
  database->markbits = newmarkbuf;
}

void db_free(GameDatabase *database) {
  char *cp;

  if (database->object_storage != nullptr) {
    for (DbRef object = 0; object < database->top; object++)
      game_object_owned_state_clear(database, object);
    cp = (char *)database->object_storage;
    free(cp);
    database->object_storage = nullptr;
  }
  free(database->markbits);
  database->markbits = nullptr;
  database->top = 0;
  database->size = 0;
  database->freelist = NOTHING;
}

DbRef parse_dbref(const char *s) {
  size_t index;
  size_t length = strlen(s);
  long x;

  /*
   * Enforce completely numeric dbrefs
   */

  for (index = 0; index < length; index++) {
    unsigned char character = *(const unsigned char *)checked_storage_at_const(
        s, length, sizeof(char), index);

    if (!(isdigit)(character))
      return NOTHING;
  }

  x = clamped_atol(s);
  return ((x >= 0) ? x : NOTHING);
}

void toast_player(EvaluationContext *evaluation, DbRef player) {
  comsys_clear_player(evaluation, player);
  del_commac(evaluation->runtime->channels, player);
  do_clear_macro(&evaluation->command->match, evaluation->runtime->macros,
                 player, nullptr);
}
