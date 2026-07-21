/* db.c - In-memory game-object and attribute database operations. */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include <assert.h>
#include <sys/file.h>
#include <sys/stat.h>

#include "mux/commands/command.h"
#include "mux/commands/macro.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/stringutil.h"
#include "mux/support/validation.h"
#include "mux/world/match.h"
#include "mux/world/object.h"
#include "mux/world/player_cache.h"
#include "mux/world/world_context.h"

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
                                 PlayerCache *players, ServerLog *log) {
  database->configuration = configuration;
  database->indexes = indexes;
  database->descriptors = descriptors;
  database->players = players;
  database->log = log;
}

void game_database_destroy(GameDatabase *database) {
  if (database == nullptr)
    return;
  db_free(database);
}

/*
 * #define GNU_MALLOC_TEST 1
 */

#ifdef GNU_MALLOC_TEST
extern unsigned int malloc_sbrk_used; /* Amount of data space used now */
#endif

/*
 * Hardcoded native fields. Dynamic Lua attributes are not registered here.
 */
Attribute attr_table[] = {{"Alias", A_ALIAS},
                          {"Buildcoord", A_BUILDCOORD},
                          {"Buildentrance", A_BUILDENTRANCE},
                          {"Buildlinks", A_BUILDLINKS},
                          {"Comment", A_COMMENT},
                          {"Contactoptions", A_CONTACTOPT},
                          {"Desc", A_DESC},
                          {"Destroyer", A_DESTROYER},
                          {"Ealias", A_EALIAS},
                          {"Faction", A_FACTION},
                          {"Idesc", A_IDESC},
                          {"Job", A_JOB},
                          {"Lalias", A_LALIAS},
                          {"Last", A_LAST},
                          {"Lastname", A_LASTNAME},
                          {"Luaparent", A_LUAPARENT},
                          {"Lastpage", A_LASTPAGE},
                          {"Lastsite", A_LASTSITE},
                          {"Logindata", A_LOGINDATA},
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
                          {"QueueMax", A_QUEUEMAX},
                          {"Ranknum", A_RANKNUM},
                          {"Semaphore", A_SEMAPHORE},
                          {"Tacsize", A_TACSIZE},
                          {"Timeout", A_TIMEOUT},
                          {"Tz", A_TZ},
                          {"Xtype", A_XTYPE},
                          {"*Password", A_PASS},
                          {"*Privileges", A_PRIVS},
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
INLINE char *game_object_name(GameDatabase *database, DbRef thing) {
  DbRef aowner;
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
      buff = attribute_get(database, thing, A_NAME, &aowner, &aflags);
      strip_ansi_r(buffer, buff, MBUF_SIZE);
      set_string(&database->pure_names[thing], buffer);
      free_lbuf(buff);
    }
  }

  attribute_get_string(database, database->name_buffer, thing, A_NAME, &aowner,
                       &aflags);
  return database->name_buffer;
}

INLINE char *game_object_pure_name(GameDatabase *database, DbRef thing) {
  DbRef aowner;
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

      buff = attribute_get(database, thing, A_NAME, &aowner, &aflags);
      set_string(&database->pure_names[thing],
                 strip_ansi_r(new, buff, strlen(buff)));
      free_lbuf(buff);
    }
    return database->pure_names[thing];
  }

  attribute_get_string(database, database->name_buffer, thing, A_NAME, &aowner,
                       &aflags);
  return strip_ansi_r(database->pure_name_buffer, database->name_buffer,
                      strlen(database->name_buffer));
}

INLINE void object_name_set(GameDatabase *database, DbRef thing, char *s) {
  char new[MBUF_SIZE];
  /* Truncate the name if we have to */

  strncpy(new, s, MBUF_SIZE - 1);
  if (s && (strlen(s) > MBUF_SIZE))
    s[MBUF_SIZE] = '\0';

  attribute_add_raw(database, thing, A_NAME, (char *)s);

  if (database->configuration->cache_names) {
    set_string(&database->pure_names[thing], strip_ansi_r(new, s, strlen(s)));
  }
}

void object_password_set(GameDatabase *database, DbRef thing, const char *s) {
  /* attribute_add_raw()'s buffer parameter isn't const-correct; s is only
     read (copied) here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  attribute_add_raw(database, thing, A_PASS, (char *)s);
#pragma clang diagnostic pop
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
 * ---------------------------------------------------------------------------
 * * mkattr: Lookup attribute by name, creating if needed.
 */

int mkattr(GameDatabase *database, char *buff) {
  Attribute *ap;

  if (!(ap = attribute_by_name(database, buff))) {
    return -1;
  }
  if (!(ap->number))
    return -1;
  return ap->number;
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
  char *text;
  if (thing < 0 || atr < 0 || atr >= 256)
    return;
  if (!buff || !*buff) {
    attribute_clear(database, thing, atr);
    return;
  }
  if (strlen(buff) >= LBUF_SIZE) {
    buff[LBUF_SIZE - 1] = '\0';
  }
  if ((text = strdup(buff)) == nullptr) {
    return;
  }
  free(database->objects[thing].native.values[atr]);
  database->objects[thing].native.values[atr] = text;
}

void attribute_add(GameDatabase *database, DbRef thing, int atr, char *buff,
                   DbRef owner, long flags) {
  (void)owner;
  (void)flags;
  attribute_add_raw(database, thing, atr, buff);
}

/*
 * ---------------------------------------------------------------------------
 * * get_atr,attribute_get_raw, attribute_get_string, attribute_get: Get an
 * attribute from the database.
 */

int get_atr(GameDatabase *database, char *name) {
  Attribute *ap;

  if (!(ap = attribute_by_name(database, name)))
    return 0;
  if (!(ap->number))
    return -1;
  return ap->number;
}

char *attribute_get_raw(GameDatabase *database, DbRef thing, int atr) {
  if (thing < 0 || atr < 0 || atr >= 256)
    return nullptr;
  return database->objects[thing].native.values[atr];
}

char *attribute_get_string(GameDatabase *database, char *s, DbRef thing,
                           int atr, DbRef *owner, long *flags) {
  char *buff;

  buff = attribute_get_raw(database, thing, atr);
  if (owner)
    *owner = game_object_owner(database, thing);
  if (flags)
    *flags = 0;
  if (!buff) {
    *s = '\0';
  } else {
    StringCopy(s, buff);
  }
  return s;
}

char *attribute_get(GameDatabase *database, DbRef thing, int atr, DbRef *owner,
                    long *flags) {
  char *buff;

  buff = alloc_lbuf("attribute_get");
  return attribute_get_string(database, buff, thing, atr, owner, flags);
}

int attribute_get_info(GameDatabase *database, DbRef thing, int atr,
                       DbRef *owner, long *flags) {
  char *buff;

  buff = attribute_get_raw(database, thing, atr);
  *owner = game_object_owner(database, thing);
  *flags = 0;
  return buff != nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * * attribute_free: Return all attributes of an object.
 */

void attribute_free(GameDatabase *database, DbRef thing) {
  for (int index = 0; index < database->objects[thing].at_count; index++) {
    free(database->objects[thing].ahead[index].name);
    free(database->objects[thing].ahead[index].data);
  }
  free(database->objects[thing].ahead);
  database->objects[thing].ahead = nullptr;
  database->objects[thing].at_count = 0;
  for (int index = 0; index < 256; index++) {
    free(database->objects[thing].native.values[index]);
    database->objects[thing].native.values[index] = nullptr;
  }
}

static int dynamic_attribute_find(const GameObject *object, const char *name,
                                  bool *found) {
  int low = 0;
  int high = object->at_count - 1;

  while (low <= high) {
    const int middle = low + ((high - low) / 2);
    const int comparison = strcmp(object->ahead[middle].name, name);
    if (comparison == 0) {
      *found = true;
      return middle;
    }
    if (comparison < 0)
      low = middle + 1;
    else
      high = middle - 1;
  }
  *found = false;
  return low;
}

const char *dynamic_attribute_get(GameDatabase *database, DbRef thing,
                                  const char *name) {
  bool found;
  int index;

  if (!is_good_obj(database, thing) || !name || !*name)
    return nullptr;
  index = dynamic_attribute_find(&database->objects[thing], name, &found);
  return found ? database->objects[thing].ahead[index].data : nullptr;
}

bool dynamic_attribute_delete(GameDatabase *database, DbRef thing,
                              const char *name) {
  GameObject *object;
  bool found;
  int index;

  if (!is_good_obj(database, thing) || !name || !*name)
    return false;
  object = &database->objects[thing];
  index = dynamic_attribute_find(object, name, &found);
  if (!found)
    return true;
  free(object->ahead[index].name);
  free(object->ahead[index].data);
  object->at_count--;
  if (index < object->at_count)
    memmove(&object->ahead[index], &object->ahead[index + 1],
            (size_t)(object->at_count - index) * sizeof(*object->ahead));
  if (object->at_count == 0) {
    free(object->ahead);
    object->ahead = nullptr;
  }
  return true;
}

bool dynamic_attribute_set(GameDatabase *database, DbRef thing,
                           const char *name, const char *value) {
  GameObject *object;
  AttributeList *entries;
  char *name_copy;
  char *value_copy;
  bool found;
  int index;

  if (!is_good_obj(database, thing) || !name || !*name ||
      strlen(name) >= SBUF_SIZE || !ok_attr_name(name))
    return false;
  if (!value || !*value)
    return dynamic_attribute_delete(database, thing, name);
  object = &database->objects[thing];
  index = dynamic_attribute_find(object, name, &found);
  value_copy = strdup(value);
  if (!value_copy)
    return false;
  if (found) {
    free(object->ahead[index].data);
    object->ahead[index].data = value_copy;
    object->ahead[index].size = (int)strlen(value_copy) + 1;
    return true;
  }
  name_copy = strdup(name);
  if (!name_copy) {
    free(value_copy);
    return false;
  }
  entries =
      realloc(object->ahead, (size_t)(object->at_count + 1) * sizeof(*entries));
  if (!entries) {
    free(name_copy);
    free(value_copy);
    return false;
  }
  object->ahead = entries;
  if (index < object->at_count)
    memmove(&entries[index + 1], &entries[index],
            (size_t)(object->at_count - index) * sizeof(*entries));
  entries[index] = (AttributeList){.name = name_copy,
                                   .data = value_copy,
                                   .size = (int)strlen(value_copy) + 1};
  object->at_count++;
  return true;
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
  for (int index = 0; index < source_object->at_count; index++)
    dynamic_attribute_set(evaluation->world->database, dest,
                          source_object->ahead[index].name,
                          source_object->ahead[index].data);
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
    game_object_set_owner(database, thing, GOD);
    game_object_set_flags(database, thing, (TYPE_GARBAGE | GOING));
    game_object_set_flags2(database, thing, 0);
    game_object_set_flags3(database, thing, 0);
    game_object_set_powers(database, thing, 0);
    game_object_set_powers2(database, thing, 0);
    game_object_set_location(database, thing, NOTHING);
    game_object_set_contents(database, thing, NOTHING);
    game_object_set_exits(database, thing, NOTHING);
    game_object_set_link(database, thing, NOTHING);
    game_object_set_next(database, thing, NOTHING);
    game_object_set_zone(database, thing, NOTHING);
    game_object_set_stack(database, thing, nullptr);
    game_database_object(database, thing)->ahead = nullptr;
    game_database_object(database, thing)->at_count = 0;
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
      game_object_set_owner(database, i, GOD);
      game_object_set_flags(database, i, (TYPE_GARBAGE | GOING));
      game_object_set_powers(database, i, 0);
      game_object_set_powers2(database, i, 0);
      game_object_set_location(database, i, NOTHING);
      game_object_set_contents(database, i, NOTHING);
      game_object_set_exits(database, i, NOTHING);
      game_object_set_link(database, i, NOTHING);
      game_object_set_next(database, i, NOTHING);
      game_object_set_zone(database, i, NOTHING);
      game_object_set_stack(database, i, nullptr);
      game_database_object(database, i)->ahead = nullptr;
      game_database_object(database, i)->at_count = 0;
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
  game_object_set_flags(database, 0, TYPE_ROOM);
  game_object_set_powers(database, 0, 0);
  game_object_set_powers2(database, 0, 0);
  game_object_set_location(database, 0, NOTHING);
  game_object_set_exits(database, 0, NOTHING);
  game_object_set_link(database, 0, NOTHING);
  game_object_set_zone(database, 0, NOTHING);
  game_object_set_owner(database, 0, 1);
  database->objects[0].ahead = nullptr;
  database->objects[0].at_count = 0;
  object_apply_default_lua_parent(evaluation, 0, TYPE_ROOM);
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
  game_object_set_flags(database, obj,
                        game_object_flags(database, obj) | WIZARD);
  game_object_set_powers(database, obj, 0);
  game_object_set_powers2(database, obj, 0);

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
    if (!isdigit(*p))
      return NOTHING;
  }

  x = clamped_atol(s);
  return ((x >= 0) ? x : NOTHING);
}

/*
 * check_zone - checks back through a zone tree for control
 */
static int check_zone_at_depth(EvaluationContext *evaluation, DbRef player,
                               DbRef thing, int depth) {
  GameDatabase *database = evaluation->world->database;
  LuaLockInvocation lock;
  LuaLockResult result;
  if (!database->configuration->have_zones ||
      (game_object_zone(database, thing) == NOTHING) ||
      (depth == database->configuration->zone_nest_lim) ||
      is_player(database, thing)) {
    return 0;
  }

  /*
   * If the zone doesn't define an enter lock, DON'T allow control.
   */

  if (lock_test(evaluation, player, player, player,
                game_object_zone(database, thing), LUA_LOCK_ENTER,
                LUA_LOCK_OPERATION_ZONE_CONTROL, true, &lock, &result) &&
      result.defined) {
    return 1;
  }
  return check_zone_at_depth(evaluation, player,
                             game_object_zone(database, thing), depth + 1);
}

int check_zone(EvaluationContext *evaluation, DbRef player, DbRef thing) {
  return check_zone_at_depth(evaluation, player, thing, 1);
}

int check_zone_for_player(EvaluationContext *evaluation, DbRef player,
                          DbRef thing) {
  GameDatabase *database = evaluation->world->database;
  LuaLockInvocation lock;
  LuaLockResult result;
  if (!database->configuration->have_zones ||
      (game_object_zone(database, thing) == NOTHING) ||
      database->configuration->zone_nest_lim == 1 ||
      !is_player(database, thing)) {
    return 0;
  }

  if (lock_test(evaluation, player, player, player,
                game_object_zone(database, thing), LUA_LOCK_ENTER,
                LUA_LOCK_OPERATION_ZONE_CONTROL, true, &lock, &result) &&
      result.defined) {
    return 1;
  }
  return check_zone_at_depth(evaluation, player,
                             game_object_zone(database, thing), 2);
}

void toast_player(EvaluationContext *evaluation, DbRef player) {
  comsys_clear_player(evaluation, player);
  del_commac(evaluation->runtime->channels, player);
  do_clear_macro(&evaluation->command->match, evaluation->runtime->macros,
                 player, nullptr);
}
