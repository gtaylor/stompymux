
/* db.h - In-memory game-object, attribute, and lock data model. */

#pragma once

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "mux/commands/command_context.h"
#include "mux/commands/command_runtime.h"
#include "mux/server/log.h" // IWYU pragma: keep
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/owned_text.h"
#include "mux/world/world_context.h"

struct GameDatabase; // IWYU pragma: keep

typedef struct Attribute Attribute;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLog ServerLog;
typedef struct StyledTextPalette StyledTextPalette;
typedef struct ObjectStateCollection ObjectStateCollection;
typedef struct PlayerAccountState PlayerAccountState;
typedef struct CharacterState CharacterState;
typedef struct WorldIndexes WorldIndexes;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct PlayerCache PlayerCache;
struct Attribute {
  const char *name;
  int number;
};

/* Native state is not part of Lua object state. The slots are
 * addressed only by the hardcoded C field selectors in attrs.h and are
 * persisted into explicit subsystem columns. */
typedef struct NativeObjectState NativeObjectState;
struct NativeObjectState {
  char *values[256];
};

typedef struct EconomyPartEntry EconomyPartEntry;
struct EconomyPartEntry {
  int part_id;
  int brand_id;
  int quantity;
};

typedef struct EconomyPartsState EconomyPartsState;
struct EconomyPartsState {
  EconomyPartEntry *entries;
  size_t count;
};

typedef struct AttributeStack AttributeStack;
struct AttributeStack {
  char *data;
  AttributeStack *next;
};

const Attribute *attribute_by_number(GameDatabase *database, int anum);
const Attribute *attribute_by_name(GameDatabase *database, const char *s);

size_t native_attribute_count(void);
const Attribute *native_attribute_at(size_t index);

constexpr char ATR_INFO_CHAR = '\1'; /* Leadin char for attr control data */

/* special dbref's */
constexpr DbRef NOTHING = -1;   /* null dbref */
constexpr DbRef AMBIGUOUS = -2; /* multiple possibilities, for matchers */
constexpr DbRef HOME = -3;      /* virtual room, represents mover's home */
constexpr DbRef NOPERM = -4;    /* Error status, no permission */

typedef enum ObjectType : int {
  OBJECT_TYPE_ROOM = 0,
  OBJECT_TYPE_THING = 1,
  OBJECT_TYPE_EXIT = 2,
  OBJECT_TYPE_PLAYER = 3,
  OBJECT_TYPE_INVALID = 4,
  OBJECT_TYPE_GARBAGE = 5,
  OBJECT_TYPE_NOTYPE = 7,
} ObjectType;

typedef struct GameObject GameObject;
struct GameObject {
  uint64_t generation; /* Changes whenever this dbref is initialized/reused. */
  DbRef location;      /* PLAYER, THING: where it is */
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
  DbRef zone; /* Whatever the object is zoned to. */

  char *lua_parent; /* Relative object_logic module path. */

  ObjectType type;

  bool has_ansi_flag;
  bool has_audible_flag;
  bool has_auditorium_flag;
  bool has_blind_flag;
  bool has_connected_flag;
  bool has_dark_flag;
  bool has_floating_flag;
  bool has_gagged_flag;
  bool has_going_flag;
  bool has_halted_flag;
  bool has_in_character_flag;
  bool has_light_flag;
  bool has_monitor_flag;
  bool has_no_command_flag;
  bool has_safe_flag;
  bool has_suspect_flag;
  bool has_transparent_flag;
  bool has_wizard_flag;
  bool has_xcode_flag;
  bool has_zombie_flag;

  bool has_idle_power;

  AttributeStack *stackhead; /* Every object has a stack. */

  ObjectStateCollection *state;
  PlayerAccountState *account; /* Present only for player objects. */
  CharacterState *character;   /* Present only for player objects. */
  EconomyPartsState economy_parts;
  NativeObjectState native;
};

typedef char *NAME;

typedef struct DatabaseMarkBuffer DatabaseMarkBuffer;
struct DatabaseMarkBuffer {
  char chunk[5000];
};

struct GameDatabase {
  GameObject *object_storage;
  NAME *name_storage;
  NAME *pure_name_storage;
  int top;
  int size;
  int minimum_size;
  int revision;
  uint64_t generation_counter;
  DbRef freelist;
  DatabaseMarkBuffer *markbits;
  ServerConfiguration *configuration;
  WorldIndexes *indexes;
  DescriptorRegistry *descriptors;
  PlayerCache *players;
  ServerLog *log;
  StyledTextPalette *styled_text_palette;
};

void game_database_initialize(GameDatabase *database);
void game_database_bind_services(GameDatabase *database,
                                 ServerConfiguration *configuration,
                                 WorldIndexes *indexes,
                                 DescriptorRegistry *descriptors,
                                 PlayerCache *players, ServerLog *log,
                                 StyledTextPalette *palette);
void game_database_destroy(GameDatabase *database);

static inline GameObject *game_database_object(GameDatabase *database,
                                               DbRef object) {
  if (database == nullptr || database->size < 0 || object < -1 ||
      object >= database->size) {
    abort();
  }
  return checked_storage_at(database->object_storage,
                            (size_t)database->size + 1, sizeof(GameObject),
                            (size_t)(object + 1));
}

static inline uint64_t game_object_generation(GameDatabase *database,
                                              DbRef object) {
  return game_database_object(database, object)->generation;
}

static inline void game_object_renew_generation(GameDatabase *database,
                                                DbRef object) {
  game_database_object(database, object)->generation =
      ++database->generation_counter;
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
static inline ObjectType game_object_type(GameDatabase *database,
                                          DbRef object) {
  return game_database_object(database, object)->type;
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
static inline void game_object_set_type(GameDatabase *database, DbRef object,
                                        ObjectType value) {
  game_database_object(database, object)->type = value;
}
static inline void game_object_set_stack(GameDatabase *database, DbRef object,
                                         AttributeStack *value) {
  game_database_object(database, object)->stackhead = value;
}

extern DbRef parse_dbref(const char * /*s*/);
extern void al_add(DbRef, int);
extern void al_delete(DbRef, int);
extern void al_destroy(DbRef);
extern void al_store(void);
extern void db_grow(GameDatabase *database, DbRef newtop);
extern void db_free(GameDatabase *database);
[[nodiscard]] bool object_password_set(GameDatabase *database, DbRef thing,
                                       const char *s);
void object_name_set(GameDatabase *database, DbRef thing, const char *s);
const char *game_object_name(GameDatabase *database, DbRef thing);
const char *game_object_pure_name(GameDatabase *database, DbRef thing);
const char *game_object_lua_parent(GameDatabase *database, DbRef object);
bool game_object_lua_parent_set(GameDatabase *database, DbRef object,
                                const char *path);
void init_min_db(void);
void attribute_stack_push(void);
void attribute_stack_pop(void);
int init_gdbm_db(char *path);
typedef struct AttributeCopyRequest {
  EvaluationContext *evaluation;
  DbRef source;
  DbRef destination;
} AttributeCopyRequest;

void attribute_copy(const AttributeCopyRequest *request);
void attribute_clear(GameDatabase *database, DbRef thing, int atr);
void attribute_add_raw(GameDatabase *database, DbRef thing, int atr,
                       const char *buff);
void attribute_add(GameDatabase *database, DbRef thing, int atr,
                   const char *buff, long flags);
char *attribute_get_raw(GameDatabase *database, DbRef thing, int atr);
OwnedText attribute_get(GameDatabase *database, DbRef thing, int atr,
                        long *flags);
char *attribute_get_string(GameDatabase *database, DbRef thing, int atr,
                           char *s, size_t size, long *flags);
bool attribute_get_info(GameDatabase *database, DbRef thing, int atr,
                        long *flags);
void attribute_free(GameDatabase *database, DbRef thing);
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
