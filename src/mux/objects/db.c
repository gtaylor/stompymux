/* db.c - In-memory game-object and attribute database operations. */

#include <ctype.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/commands/macro.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/objects/attrs.h"
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
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/utf8.h"
#include "mux/support/validation.h"
#include "mux/world/object.h"
#include "mux/world/player.h"

#ifndef O_ACCMODE
#define O_ACCMODE (O_RDONLY | O_WRONLY | O_RDWR)
#endif

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

/*
 * Hardcoded native fields. Dynamic Lua attributes are not registered here.
 */
Attribute attr_table[] = {{"Alias", A_ALIAS},
                          {"Buildcoord", A_BUILDCOORD},
                          {"Buildentrance", A_BUILDENTRANCE},
                          {"Buildlinks", A_BUILDLINKS},
                          {"Contactoptions", A_CONTACTOPT},
                          {"Desc", A_DESC},
                          {"Destroyer", A_DESTROYER},
                          {"Faction", A_FACTION},
                          {"Idesc", A_IDESC},
                          {"LRSheight", A_LRSHEIGHT},
                          {"Mapvis", A_MAPVIS},
                          {"Mechdesc", A_MECHDESC},
                          {"Mechname", A_MECHNAME},
                          {"Mechtype", A_MECHTYPE},
                          {"MechPrefID", A_MECHPREFID},
                          {"Mechskills", A_MECHSKILLS},
                          {"Mwtemplate", A_MWTEMPLATE},
                          {"PCequip", A_PCEQUIP},
                          {"Pilot", A_PILOTNUM},
                          {"Tacsize", A_TACSIZE},
                          {"Xtype", A_XTYPE},
                          {nullptr, 0}};

size_t native_attribute_count(void) {
  return sizeof(attr_table) / sizeof(attr_table[0]) - 1;
}

Attribute *native_attribute_at(size_t index) {
  return checked_storage_at(attr_table, native_attribute_count(),
                            sizeof(*attr_table), index);
}

static char *set_string(char **ptr, char *new) {
  /*
   * if pointer not null unalloc it
   */

  if (*ptr)
    free(*ptr);

  /*
   * if new string is not null allocate space for it and copy it
   */

  if (!new)                  /*
                              * * || !*new
                              */
    return (*ptr = nullptr); /*
                              * Check with GAC about this
                              */
  *ptr = (char *)malloc(strlen(new) + 1);
  StringCopy(*ptr, new);
  return (*ptr);
}

static NAME *pure_name_slot(GameDatabase *database, DbRef object) {
  if (database == nullptr || database->size < 0 || object < -1 ||
      object >= database->size) {
    abort();
  }
  return checked_storage_at(database->pure_name_storage,
                            (size_t)database->size + 1, sizeof(NAME),
                            (size_t)(object + 1));
}

static char **native_attribute_slot(GameDatabase *database, DbRef object,
                                    int attribute) {
  if (attribute < 0 || attribute >= 256)
    return nullptr;
  GameObject *game_object = game_database_object(database, object);
  return checked_storage_at(game_object->native.values, 256, sizeof(char *),
                            (size_t)attribute);
}

/*
 * ---------------------------------------------------------------------------
 * * Name, s_Name: Get or set an object's name.
 */
char *game_object_name(GameDatabase *database, DbRef thing) {
  long aflags;
  char *buff;
  char buffer[MBUF_SIZE];

  if (database->configuration->cache_names) {
    if (thing > database->top || thing < 0) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      return (char *)"#-1 INVALID DBREF";
#pragma clang diagnostic pop
    }
    if (!*pure_name_slot(database, thing)) {
      buff = attribute_get(database, thing, A_NAME, &aflags);
      styled_text_strip(database->styled_text_palette, buff, buffer, MBUF_SIZE);
      set_string(pure_name_slot(database, thing), buffer);
      free_lbuf(buff);
    }
  }

  attribute_get_string(database, database->name_buffer, thing, A_NAME, &aflags);
  return database->name_buffer;
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

char *game_object_pure_name(GameDatabase *database, DbRef thing) {
  long aflags;
  char *buff;

  if (database->configuration->cache_names) {
    if (thing > database->top || thing < 0) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      return (char *)"#-1 INVALID DBREF";
#pragma clang diagnostic pop
    }
    if (!*pure_name_slot(database, thing)) {
      char new[LBUF_SIZE];

      buff = attribute_get(database, thing, A_NAME, &aflags);
      styled_text_strip(database->styled_text_palette, buff, new, sizeof(new));
      set_string(pure_name_slot(database, thing), new);
      free_lbuf(buff);
    }
    return *pure_name_slot(database, thing);
  }

  attribute_get_string(database, database->name_buffer, thing, A_NAME, &aflags);
  styled_text_strip(database->styled_text_palette, database->name_buffer,
                    database->pure_name_buffer,
                    sizeof(database->pure_name_buffer));
  return database->pure_name_buffer;
}

void object_name_set(GameDatabase *database, DbRef thing, char *s) {
  char stored[MBUF_SIZE];
  char new[MBUF_SIZE];

  utf8_copy_truncated(stored, sizeof(stored), s);
  attribute_add_raw(database, thing, A_NAME, stored);

  if (database->configuration->cache_names) {
    styled_text_strip(database->styled_text_palette, stored, new, sizeof(new));
    set_string(pure_name_slot(database, thing), new);
  }
}

void object_password_set(GameDatabase *database, DbRef thing, const char *s) {
  player_account_password_hash_set(database, thing, s);
}

Attribute *attribute_by_name(GameDatabase *database, const char *s) {
  (void)database;
  if (s == nullptr || *s == '\0')
    return nullptr;
  for (size_t index = 0; index < native_attribute_count(); index++) {
    Attribute *attribute = native_attribute_at(index);

    if (strcasecmp(attribute->name, s) == 0)
      return attribute;
  }
  return nullptr;
}

Attribute *attribute_by_number(GameDatabase *database, int anum) {
  (void)database;
  for (size_t index = 0; index < native_attribute_count(); index++) {
    Attribute *attribute = native_attribute_at(index);

    if (attribute->number == anum)
      return attribute;
  }
  return nullptr;
}

/*
 * routines to handle object attribute lists
 */

/*
 * ---------------------------------------------------------------------------
 * * attribute_clear: clear an attribute in the list.
 */

void attribute_clear(GameDatabase *database, DbRef thing, int atr) {
  if (thing < 0 || atr < 0 || atr >= 256)
    return;
  char **slot = native_attribute_slot(database, thing, atr);
  free(*slot);
  *slot = nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_add_raw, attribute_add: add attribute of type atr to list
 */

void attribute_add_raw(GameDatabase *database, DbRef thing, int atr,
                       const char *buff) {
  char truncated[LBUF_SIZE];
  char *text;
  if (thing < 0 || atr < 0 || atr >= 256)
    return;
  if (!buff || !*buff) {
    attribute_clear(database, thing, atr);
    return;
  }
  utf8_copy_truncated(truncated, sizeof(truncated), buff);
  if ((text = strdup(truncated)) == nullptr) {
    return;
  }
  char **slot = native_attribute_slot(database, thing, atr);
  free(*slot);
  *slot = text;
}

void attribute_add(GameDatabase *database, DbRef thing, int atr,
                   const char *buff, long flags) {
  (void)flags;
  attribute_add_raw(database, thing, atr, buff);
}

/*
 * ---------------------------------------------------------------------------
 * * get_atr,attribute_get_raw, attribute_get_string, attribute_get: Get an
 * attribute from the database.
 */

char *attribute_get_raw(GameDatabase *database, DbRef thing, int atr) {
  if (thing < 0 || atr < 0 || atr >= 256)
    return nullptr;
  return *native_attribute_slot(database, thing, atr);
}

char *attribute_get_string(GameDatabase *database, char *s, DbRef thing,
                           int atr, long *flags) {
  char *buff;

  buff = attribute_get_raw(database, thing, atr);
  if (flags)
    *flags = 0;
  if (!buff) {
    *s = '\0';
  } else {
    StringCopy(s, buff);
  }
  return s;
}

char *attribute_get(GameDatabase *database, DbRef thing, int atr, long *flags) {
  char *buff;

  buff = alloc_lbuf("attribute_get");
  return attribute_get_string(database, buff, thing, atr, flags);
}

int attribute_get_info(GameDatabase *database, DbRef thing, int atr,
                       long *flags) {
  char *buff;

  buff = attribute_get_raw(database, thing, atr);
  *flags = 0;
  return buff != nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_free: Return all attributes of an object.
 */

void attribute_free(GameDatabase *database, DbRef thing) {
  GameObject *game_object = game_database_object(database, thing);
  free(game_object->lua_parent);
  game_object->lua_parent = nullptr;
  object_state_clear(database, thing);
  player_account_clear(database, thing);
  character_state_clear(database, thing);
  economy_parts_clear(database, thing);
  for (int index = 0; index < 256; index++) {
    char **slot = native_attribute_slot(database, thing, index);
    free(*slot);
    *slot = nullptr;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_copy: Copy all attributes from one object to another.  Takes the
 * * player argument to ensure that only attributes that COULD be set by
 * * the player are copied.
 */

void attribute_copy(EvaluationContext *evaluation, DbRef player, DbRef dest,
                    DbRef source) {
  (void)player;
  GameObject *source_object =
      game_database_object(evaluation->world->database, source);
  for (int field = 1; field < 256; field++) {
    const char *value =
        *native_attribute_slot(evaluation->world->database, source, field);
    if (value)
      attribute_add_raw(evaluation->world->database, dest, field, value);
  }
  object_state_copy(evaluation->world->database, dest, source);
  game_object_lua_parent_set(evaluation->world->database, dest,
                             source_object->lua_parent);
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * db_grow: Extend the struct database.
 */

// So mistaken refs to #-1 won't die.
constexpr int SIZE_HACK = 1;

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
    game_object_set_stack(database, thing, nullptr);
    game_database_object(database, thing)->state = nullptr;
  }
}

void db_grow(GameDatabase *database, DbRef newtop) {
  int newsize, marksize, delta, i;
  DatabaseMarkBuffer *newmarkbuf;
  GameObject *newdb;
  NAME *newpurenames;

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
    for (i = database->top; i < newtop; i++) {
      if (database->configuration->cache_names)
        *pure_name_slot(database, i) = nullptr;
    }
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
  ;

  /*
   * Grow the name tables
   */

  if (database->configuration->cache_names) {
    newpurenames = (NAME *)malloc((size_t)(newsize + SIZE_HACK) * sizeof(NAME));

    if (!newpurenames) {
      log_simple(
          database->log, LOG_ALWAYS, "ALC", "DB",
          tprintf("Could not allocate space for %d item name cache.", newsize));
      abort();
    }
    bzero((char *)newpurenames, (size_t)(newsize + SIZE_HACK) * sizeof(NAME));

    if (database->pure_name_storage) {

      /*
       * An old name cache exists.  Copy it.
       */

      bcopy((char *)database->pure_name_storage, (char *)newpurenames,
            (size_t)(newtop + SIZE_HACK) * sizeof(NAME));
      cp = (char *)database->pure_name_storage;
      free(cp);
    } else {

      /*
       * Creating a brand new struct database.  Fill in the
       * 'reserved' area in case it gets referenced.
       */

      database->pure_name_storage = newpurenames;
      for (i = 0; i < SIZE_HACK; i++) {
        *pure_name_slot(database, i - SIZE_HACK) = nullptr;
      }
    }
    database->pure_name_storage = newpurenames;
    newpurenames = nullptr;
  }
  /*
   * Grow the database->objects array
   */

  newdb =
      (GameObject *)malloc((size_t)(newsize + SIZE_HACK) * sizeof(GameObject));
  if (!newdb) {

    log_simple(database->log, LOG_ALWAYS, "ALC", "DB",
               tprintf("Could not allocate space for %d item struct database.",
                       newsize));
    abort();
  }
  database->size = newsize;
  if (database->object_storage) {

    /*
     * An old struct database exists.  Copy it to the new buffer
     */

    bcopy((char *)database->object_storage, (char *)newdb,
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
      const DbRef reserved = i - SIZE_HACK;
      game_object_set_type(database, reserved, OBJECT_TYPE_GARBAGE);
      game_object_clear_flags(database, reserved);
      s_going(database, reserved);
      game_object_clear_powers(database, reserved);
      game_object_set_location(database, reserved, NOTHING);
      game_object_set_contents(database, reserved, NOTHING);
      game_object_set_exits(database, reserved, NOTHING);
      game_object_set_link(database, reserved, NOTHING);
      game_object_set_next(database, reserved, NOTHING);
      game_object_set_zone(database, reserved, NOTHING);
      game_object_set_stack(database, reserved, nullptr);
      game_database_object(database, reserved)->state = nullptr;
    }
  }
  database->object_storage = newdb;
  newdb = nullptr;

  for (i = database->top; i < newtop; i++) {
    if (database->configuration->cache_names) {
      *pure_name_slot(database, i) = nullptr;
    }
  }
  initialize_objects(database, database->top, newtop);
  database->top = (int)newtop;

  /*
   * Grow the database->objects mark buffer
   */

  marksize = (newsize + 7) >> 3;
  newmarkbuf = (DatabaseMarkBuffer *)malloc((size_t)marksize);
  bzero((char *)newmarkbuf, (size_t)marksize);
  if (database->markbits) {
    marksize = (int)((newtop + 7) >> 3);
    bcopy((char *)database->markbits, (char *)newmarkbuf, (size_t)marksize);
    cp = (char *)database->markbits;
    free(cp);
  }
  database->markbits = newmarkbuf;
}

void db_free(GameDatabase *database) {
  char *cp;

  if (database->object_storage != nullptr) {
    for (DbRef object = 0; object < database->top; object++)
      attribute_free(database, object);
    cp = (char *)database->object_storage;
    free(cp);
    database->object_storage = nullptr;
  }
  if (database->pure_name_storage != nullptr) {
    for (DbRef object = 0; object < database->top; object++)
      free(*pure_name_slot(database, object));
    free(database->pure_name_storage);
    database->pure_name_storage = nullptr;
  }
  free(database->markbits);
  database->markbits = nullptr;
  database->top = 0;
  database->size = 0;
  database->freelist = NOTHING;
}

void db_make_minimal(EvaluationContext *evaluation) {
  GameDatabase *database = evaluation->world->database;
  DbRef obj;

  db_free(database);
  db_grow(database, 1);
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  object_name_set(database, 0, (char *)"Limbo");
#pragma clang diagnostic pop
  game_object_set_type(database, 0, OBJECT_TYPE_ROOM);
  game_object_clear_flags(database, 0);
  game_object_clear_powers(database, 0);
  game_object_set_location(database, 0, NOTHING);
  game_object_set_exits(database, 0, NOTHING);
  game_object_set_link(database, 0, NOTHING);
  game_object_set_zone(database, 0, NOTHING);
  game_database_object(database, 0)->state = nullptr;
  object_apply_default_lua_parent(evaluation, 0, OBJECT_TYPE_ROOM);
  /*
   * should be #1
   */
  load_player_names(evaluation->world);
  /* create_player()'s parameters aren't const-correct; these literals are
     only read here. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  obj = create_player(evaluation, (char *)"Wizard", (char *)"potrzebie");
#pragma clang diagnostic pop
  game_object_set_flag(database, obj, OBJECT_FLAG_WIZARD, true);
  game_object_clear_powers(database, obj);

  /*
   * Manually link to Limbo, just in case
   */
  game_object_set_location(database, obj, 0);
  game_object_set_next(database, obj, NOTHING);
  game_object_set_contents(database, 0, obj);
  game_object_set_link(database, obj, 0);
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
