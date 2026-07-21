
/* db.h - In-memory game-object, attribute, and lock data model. */

#pragma once

#include "mux/server/platform.h"
#include "mux/support/alloc.h"

#include <sys/file.h>

typedef struct Attribute Attribute;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;
typedef struct WorldIndexes WorldIndexes;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct PlayerCache PlayerCache;
int get_atr(GameDatabase *database, char *name);
struct Attribute {
  const char *name;
  int number;
};

typedef struct AttributeList AttributeList;
struct AttributeList {
  char *name; /* Exact, case-sensitive dynamic storage key. */
  char *data; /* Attribute text. */
  int size;   /* Length of attribute */
};

/* Native state is not part of the dynamic attribute namespace.  The slots are
 * addressed only by the hardcoded C field selectors in attrs.h and are
 * persisted into explicit subsystem columns. */
typedef struct NativeObjectState NativeObjectState;
struct NativeObjectState {
  char *values[256];
};

typedef struct AttributeStack AttributeStack;
struct AttributeStack {
  char *data;
  AttributeStack *next;
};

extern Attribute *attribute_by_number(GameDatabase *database, int anum);
extern Attribute *attribute_by_name(GameDatabase *database, const char *s);

extern Attribute attr_table[];

constexpr char ATR_INFO_CHAR = '\1'; /* Leadin char for attr control data */

/* special dbref's */
constexpr DbRef NOTHING = -1;   /* null dbref */
constexpr DbRef AMBIGUOUS = -2; /* multiple possibilities, for matchers */
constexpr DbRef HOME = -3;      /* virtual room, represents mover's home */
constexpr DbRef NOPERM = -4;    /* Error status, no permission */

typedef struct GameObject GameObject;
struct GameObject {
  DbRef location; /* PLAYER, THING: where it is */
  /* ROOM: dropto: */
  /* EXIT: where it goes to */
  DbRef contents; /* PLAYER, THING, ROOM: head of contentslist */
  /* EXIT: unused */
  DbRef exits; /* PLAYER, THING, ROOM: head of exitslist */
  /* EXIT: where it is */
  DbRef next; /* PLAYER, THING: next in contentslist */
  /* EXIT: next in exitslist */
  /* ROOM: unused */
  DbRef link; /* PLAYER, THING: home location */
  /* ROOM, EXIT: unused */
  DbRef owner; /* PLAYER: domain number + class + moreflags */
  /* THING, ROOM, EXIT: owning player number */

  DbRef zone; /* Whatever the object is zoned to. */

  Flag flags;  /* ALL: Flags set on the object */
  Flag flags2; /* ALL: even more flags */
  Flag flags3; /* ALL: yet _more_ flags */

  Power powers;  /* ALL: Powers on object */
  Power powers2; /* ALL: even more powers */

  AttributeStack *stackhead; /* Every object has a stack. */

  AttributeList *ahead; /* The head of the attribute list. */
  int at_count;         /* How many attributes do we have? */
  NativeObjectState native;
};

typedef char *NAME;

typedef struct DatabaseMarkBuffer DatabaseMarkBuffer;
struct DatabaseMarkBuffer {
  char chunk[5000];
};

struct GameDatabase {
  GameObject *objects;
  NAME *pure_names;
  char name_buffer[MBUF_SIZE];
  char pure_name_buffer[LBUF_SIZE];
  int top;
  int size;
  int minimum_size;
  int revision;
  DbRef freelist;
  DatabaseMarkBuffer *markbits;
  ServerConfiguration *configuration;
  WorldIndexes *indexes;
  DescriptorRegistry *descriptors;
  PlayerCache *players;
  ServerLog *log;
};

void game_database_initialize(GameDatabase *database);
void game_database_bind_services(GameDatabase *database,
                                 ServerConfiguration *configuration,
                                 WorldIndexes *indexes,
                                 DescriptorRegistry *descriptors,
                                 PlayerCache *players, ServerLog *log);
void game_database_destroy(GameDatabase *database);

static inline GameObject *game_database_object(GameDatabase *database,
                                               DbRef object) {
  return &database->objects[object];
}

static inline DbRef game_object_location(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->location;
}
static inline DbRef game_object_zone(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->zone;
}
static inline DbRef game_object_contents(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->contents;
}
static inline DbRef game_object_exits(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->exits;
}
static inline DbRef game_object_next(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->next;
}
static inline DbRef game_object_link(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->link;
}
static inline DbRef game_object_owner(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->owner;
}
static inline Flag game_object_flags(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->flags;
}
static inline Flag game_object_flags2(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->flags2;
}
static inline Flag game_object_flags3(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->flags3;
}
static inline Power game_object_powers(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->powers;
}
static inline Power game_object_powers2(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->powers2;
}
static inline AttributeStack *game_object_stack(GameDatabase *database,
                                                DbRef object) {
  return game_database_object(database, object)->stackhead;
}

static inline void game_object_set_location(GameDatabase *database,
                                            DbRef object, DbRef value) {
  game_database_object(database, object)->location = value;
}
static inline void game_object_set_zone(GameDatabase *database, DbRef object,
                                        DbRef value) {
  game_database_object(database, object)->zone = value;
}
static inline void game_object_set_contents(GameDatabase *database,
                                            DbRef object, DbRef value) {
  game_database_object(database, object)->contents = value;
}
static inline void game_object_set_exits(GameDatabase *database, DbRef object,
                                         DbRef value) {
  game_database_object(database, object)->exits = value;
}
static inline void game_object_set_next(GameDatabase *database, DbRef object,
                                        DbRef value) {
  game_database_object(database, object)->next = value;
}
static inline void game_object_set_link(GameDatabase *database, DbRef object,
                                        DbRef value) {
  game_database_object(database, object)->link = value;
}
static inline void game_object_set_owner(GameDatabase *database, DbRef object,
                                         DbRef value) {
  game_database_object(database, object)->owner = value;
}
static inline void game_object_set_flags(GameDatabase *database, DbRef object,
                                         Flag value) {
  game_database_object(database, object)->flags = value;
}
static inline void game_object_set_flags2(GameDatabase *database, DbRef object,
                                          Flag value) {
  game_database_object(database, object)->flags2 = value;
}
static inline void game_object_set_flags3(GameDatabase *database, DbRef object,
                                          Flag value) {
  game_database_object(database, object)->flags3 = value;
}
static inline void game_object_set_powers(GameDatabase *database, DbRef object,
                                          Power value) {
  game_database_object(database, object)->powers = value;
}
static inline void game_object_set_powers2(GameDatabase *database, DbRef object,
                                           Power value) {
  game_database_object(database, object)->powers2 = value;
}
static inline void game_object_set_stack(GameDatabase *database, DbRef object,
                                         AttributeStack *value) {
  game_database_object(database, object)->stackhead = value;
}

extern DbRef parse_dbref(const char *);
extern int mkattr(GameDatabase *database, char *name);
extern void al_add(DbRef, int);
extern void al_delete(DbRef, int);
extern void al_destroy(DbRef);
extern void al_store(void);
extern void db_grow(GameDatabase *database, DbRef newtop);
extern void db_free(GameDatabase *database);
extern void db_make_minimal(EvaluationContext *evaluation);
void object_password_set(GameDatabase *database, DbRef thing,
                         const char *password);
void object_name_set(GameDatabase *database, DbRef thing, char *name);
char *game_object_name(GameDatabase *database, DbRef thing);
char *game_object_pure_name(GameDatabase *database, DbRef thing);
void init_min_db(void);
void attribute_stack_push(void);
void attribute_stack_pop(void);
int init_gdbm_db(char *path);
void attribute_copy(EvaluationContext *evaluation, DbRef player, DbRef source,
                    DbRef destination);
void attribute_clear(GameDatabase *database, DbRef thing, int attribute_number);
void attribute_add_raw(GameDatabase *database, DbRef thing,
                       int attribute_number, char *value);
void attribute_add(GameDatabase *database, DbRef thing, int attribute_number,
                   char *value, DbRef owner, long flags);
char *attribute_get_raw(GameDatabase *database, DbRef thing,
                        int attribute_number);
char *attribute_get(GameDatabase *database, DbRef thing, int attribute_number,
                    DbRef *owner, long *flags);
char *attribute_get_string(GameDatabase *database, char *buffer, DbRef thing,
                           int attribute_number, DbRef *owner, long *flags);
int attribute_get_info(GameDatabase *database, DbRef thing,
                       int attribute_number, DbRef *owner, long *flags);
void attribute_free(GameDatabase *database, DbRef thing);
const char *dynamic_attribute_get(GameDatabase *database, DbRef thing,
                                  const char *name);
bool dynamic_attribute_set(GameDatabase *database, DbRef thing,
                           const char *name, const char *value);
bool dynamic_attribute_delete(GameDatabase *database, DbRef thing,
                              const char *name);
int check_zone(EvaluationContext *evaluation, DbRef player, DbRef thing);
int check_zone_for_player(EvaluationContext *evaluation, DbRef player,
                          DbRef thing);
void toast_player(EvaluationContext *evaluation, DbRef player);

#define DOLIST(database, thing, list)                                          \
  for ((thing) = (list); ((thing) != NOTHING) &&                               \
                         (game_object_next((database), (thing)) != (thing));   \
       (thing) = game_object_next((database), (thing)))
#define SAFE_DOLIST(database, thing, next, list)                               \
  for ((thing) = (list),                                                       \
      (next) = ((thing) == NOTHING ? NOTHING                                   \
                                   : game_object_next((database), (thing)));   \
       (thing) != NOTHING &&                                                   \
       (game_object_next((database), (thing)) != (thing));                     \
       (thing) = (next),                                                       \
      (next) = ((next) == NOTHING ? NOTHING                                    \
                                  : game_object_next((database), (next))))
#define DO_WHOLE_DB(database, thing)                                           \
  for ((thing) = 0; (thing) < (database)->top; (thing)++)

#define DO_WHOLE_DB_REV(database, thing)                                       \
  for ((thing) = (database)->top - 1; (thing) > 0; (thing)--)

constexpr int DUMP_NORMAL = 0;
constexpr int DUMP_CRASHED = 1;
constexpr int DUMP_KILLED = 4;
