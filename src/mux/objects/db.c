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
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/player_account.h"
#include "mux/objects/powers.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
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
                          {"Mapcolor", A_MAPCOLOR},
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
                          {"Techtime", A_TECHTIME},
                          {"*EconParts", A_ECONPARTS},
                          {"PLHEALTH", A_HEALTH},
                          {"PLATTRS", A_ATTRS},
                          {"PLADVS", A_ADVS},
                          {"PLSKILLS", A_SKILLS},
                          {nullptr, 0}};

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
    if (!database->pure_names[thing]) {
      buff = attribute_get(database, thing, A_NAME, &aflags);
      styled_text_strip(database->styled_text_palette, buff, buffer, MBUF_SIZE);
      set_string(&database->pure_names[thing], buffer);
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
    if (!database->pure_names[thing]) {
      char new[LBUF_SIZE];

      buff = attribute_get(database, thing, A_NAME, &aflags);
      styled_text_strip(database->styled_text_palette, buff, new, sizeof(new));
      set_string(&database->pure_names[thing], new);
      free_lbuf(buff);
    }
    return database->pure_names[thing];
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
    set_string(&database->pure_names[thing], new);
  }
}

void object_password_set(GameDatabase *database, DbRef thing, const char *s) {
  player_account_password_hash_set(database, thing, s);
}

Attribute *attribute_by_name(GameDatabase *database, const char *s) {
  (void)database;
  if (s == nullptr || *s == '\0')
    return nullptr;
  for (Attribute *attribute = attr_table; attribute->number != 0; attribute++) {
    if (strcasecmp(attribute->name, s) == 0)
      return attribute;
  }
  return nullptr;
}

Attribute *attribute_by_number(GameDatabase *database, int anum) {
  (void)database;
  for (Attribute *attribute = attr_table; attribute->number != 0; attribute++) {
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
  free(database->objects[thing].native.values[atr]);
  database->objects[thing].native.values[atr] = nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_add_raw, attribute_add: add attribute of type atr to list
 */

void attribute_add_raw(GameDatabase *database, DbRef thing, int atr,
                       char *buff) {
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
  free(database->objects[thing].native.values[atr]);
  database->objects[thing].native.values[atr] = text;
}

void attribute_add(GameDatabase *database, DbRef thing, int atr, char *buff,
                   long flags) {
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
  return database->objects[thing].native.values[atr];
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
  free(database->objects[thing].lua_parent);
  database->objects[thing].lua_parent = nullptr;
  object_state_clear(database, thing);
  player_account_clear(database, thing);
  for (int index = 0; index < 256; index++) {
    free(database->objects[thing].native.values[index]);
    database->objects[thing].native.values[index] = nullptr;
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
    const char *value = source_object->native.values[field];
    if (value)
      attribute_add_raw(evaluation->world->database, dest, field,
                        source_object->native.values[field]);
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
        database->pure_names[i] = nullptr;
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

    if (database->pure_names) {

      /*
       * An old name cache exists.  Copy it.
       */

      database->pure_names -= SIZE_HACK;
      bcopy((char *)database->pure_names, (char *)newpurenames,
            (size_t)(newtop + SIZE_HACK) * sizeof(NAME));
      cp = (char *)database->pure_names;
      free(cp);
    } else {

      /*
       * Creating a brand new struct database.  Fill in the
       * 'reserved' area in case it gets referenced.
       */

      database->pure_names = newpurenames;
      for (i = 0; i < SIZE_HACK; i++) {
        database->pure_names[i] = nullptr;
      }
    }
    database->pure_names = newpurenames + SIZE_HACK;
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
  if (database->objects) {

    /*
     * An old struct database exists.  Copy it to the new buffer
     */

    database->objects -= SIZE_HACK;
    bcopy((char *)database->objects, (char *)newdb,
          (size_t)(database->top + SIZE_HACK) * sizeof(GameObject));
    cp = (char *)database->objects;
    free(cp);
  } else {

    /*
     * Creating a brand new struct database.  Fill in the * * * *
     *
     * *  * * 'reserved' area in case it gets referenced.
     */

    database->objects = newdb;
    for (i = 0; i < SIZE_HACK; i++) {
      game_object_set_type(database, i, OBJECT_TYPE_GARBAGE);
      game_object_clear_flags(database, i);
      s_going(database, i);
      game_object_clear_powers(database, i);
      game_object_set_location(database, i, NOTHING);
      game_object_set_contents(database, i, NOTHING);
      game_object_set_exits(database, i, NOTHING);
      game_object_set_link(database, i, NOTHING);
      game_object_set_next(database, i, NOTHING);
      game_object_set_zone(database, i, NOTHING);
      game_object_set_stack(database, i, nullptr);
      game_database_object(database, i)->state = nullptr;
    }
  }
  database->objects = newdb + SIZE_HACK;
  newdb = nullptr;

  for (i = database->top; i < newtop; i++) {
    if (database->configuration->cache_names) {
      database->pure_names[i] = nullptr;
    }
  }
  initialize_objects(database, database->top, newtop);
  database->top = (int)newtop;
  database->size = newsize;

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

  if (database->objects != nullptr) {
    for (DbRef object = 0; object < database->top; object++)
      attribute_free(database, object);
    database->objects -= SIZE_HACK;
    cp = (char *)database->objects;
    free(cp);
    database->objects = nullptr;
  }
  if (database->pure_names != nullptr) {
    for (DbRef object = 0; object < database->top; object++)
      free(database->pure_names[object]);
    database->pure_names -= SIZE_HACK;
    free(database->pure_names);
    database->pure_names = nullptr;
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
  database->objects[0].state = nullptr;
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
  const char *p;
  long x;

  /*
   * Enforce completely numeric dbrefs
   */

  for (p = s; *p; p++) {
    if (!isdigit((unsigned char)*p))
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
