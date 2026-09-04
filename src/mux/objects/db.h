/** @file
 * In-memory game-object, attribute, and lock data model.
 */
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

/* special dbref's */
constexpr DbRef NOTHING = -1;   /* null dbref */
constexpr DbRef AMBIGUOUS = -2; /* multiple possibilities, for matchers */
/* Dbref -3 is intentionally unused. */
constexpr DbRef NOPERM = -4; /* Error status, no permission */

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
  DbRef zone;              /* Whatever the object is zoned to. */
  DbRef affiliation;       /* Object this object is affiliated with. */
  DbRef pending_destroyer; /* Actor responsible for deferred destruction. */

  char *name;
  char *pure_name;
  char *description;
  char *internal_description;
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
  bool has_zombie_flag;

  bool has_idle_power;

  AttributeStack *stackhead; /* Every object has a stack. */

  ObjectStateCollection *state;
  PlayerAccountState *account; /* Present only for player objects. */
  CharacterState *character;   /* Present only for player objects. */
  EconomyPartsState economy_parts;
};

typedef struct DatabaseMarkBuffer DatabaseMarkBuffer;
struct DatabaseMarkBuffer {
  char chunk[5000];
};

struct GameDatabase {
  GameObject *object_storage;
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

/** Initializes game database. @param[out] database Game database. */

void game_database_initialize(GameDatabase *database);
/** Executes game database bind services. @param[in,out] database Game database.
 * @param[in,out] configuration Server configuration. @param[in,out] indexes
 * Indexes. @param[in,out] descriptors Descriptors. @param[in,out] players
 * Players. @param[in,out] log Server log. @param[in,out] palette Palette. */

void game_database_bind_services(GameDatabase *database,
                                 ServerConfiguration *configuration,
                                 WorldIndexes *indexes,
                                 DescriptorRegistry *descriptors,
                                 PlayerCache *players, ServerLog *log,
                                 StyledTextPalette *palette);
/** Destroys game database. @param[in,out] database Game database. */

void game_database_destroy(GameDatabase *database);

/** Executes game database object. @param[in,out] database Game database.
 * @param[in] object Game object. */

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

/** Executes game object generation. @param[in,out] database Game database.
 * @param[in] object Game object. */

static inline uint64_t game_object_generation(GameDatabase *database,
                                              DbRef object) {
  return game_database_object(database, object)->generation;
}

/** Executes game object renew generation. @param[in,out] database Game
 * database. @param[in] object Game object. */

static inline void game_object_renew_generation(GameDatabase *database,
                                                DbRef object) {
  game_database_object(database, object)->generation =
      ++database->generation_counter;
}

/** Executes game object location. @param[in,out] database Game database.
 * @param[in] object Game object. */

static inline DbRef game_object_location(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->location;
}
/** Executes game object zone. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline DbRef game_object_zone(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->zone;
}
/** Returns a game object's affiliation. @param[in,out] database Game database.
 * @param[in] object Game object. */

static inline DbRef game_object_affiliation(GameDatabase *database,
                                            DbRef object) {
  return game_database_object(database, object)->affiliation;
}
/** Executes game object contents. @param[in,out] database Game database.
 * @param[in] object Game object. */

static inline DbRef game_object_contents(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->contents;
}
/** Executes game object exits. @param[in,out] database Game database.
 * @param[in] object Game object. */

static inline DbRef game_object_exits(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->exits;
}
/** Executes game object next. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline DbRef game_object_next(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->next;
}
/** Executes game object link. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline DbRef game_object_link(GameDatabase *database, DbRef object) {
  return game_database_object(database, object)->link;
}
/** Executes game object type. @param[in] database Game database. @param[in]
 * object Game object. */

static inline ObjectType game_object_type(GameDatabase *database,
                                          DbRef object) {
  return game_database_object(database, object)->type;
}
/** Executes game object stack. @param[in,out] database Game database.
 * @param[in] object Game object. */

static inline AttributeStack *game_object_stack(GameDatabase *database,
                                                DbRef object) {
  return game_database_object(database, object)->stackhead;
}

/** Sets location on game object. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in] value Value to use. */

static inline void game_object_set_location(GameDatabase *database,
                                            DbRef object, DbRef value) {
  game_database_object(database, object)->location = value;
}
/** Sets zone on game object. @param[in,out] database Game database. @param[in]
 * object Game object. @param[in] value Value to use. */

static inline void game_object_set_zone(GameDatabase *database, DbRef object,
                                        DbRef value) {
  game_database_object(database, object)->zone = value;
}
/** Sets a game object's affiliation. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in] value Affiliation object or
 * NOTHING. */

static inline void game_object_set_affiliation(GameDatabase *database,
                                               DbRef object, DbRef value) {
  game_database_object(database, object)->affiliation = value;
}
/** Sets contents on game object. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in] value Value to use. */

static inline void game_object_set_contents(GameDatabase *database,
                                            DbRef object, DbRef value) {
  game_database_object(database, object)->contents = value;
}
/** Sets exits on game object. @param[in,out] database Game database. @param[in]
 * object Game object. @param[in] value Value to use. */

static inline void game_object_set_exits(GameDatabase *database, DbRef object,
                                         DbRef value) {
  game_database_object(database, object)->exits = value;
}
/** Sets next on game object. @param[in,out] database Game database. @param[in]
 * object Game object. @param[in] value Value to use. */

static inline void game_object_set_next(GameDatabase *database, DbRef object,
                                        DbRef value) {
  game_database_object(database, object)->next = value;
}
/** Sets link on game object. @param[in,out] database Game database. @param[in]
 * object Game object. @param[in] value Value to use. */

static inline void game_object_set_link(GameDatabase *database, DbRef object,
                                        DbRef value) {
  game_database_object(database, object)->link = value;
}
/** Sets type on game object. @param[in] database Game database. @param[in]
 * object Game object. @param[in] value Value to use. */

static inline void game_object_set_type(GameDatabase *database, DbRef object,
                                        ObjectType value) {
  game_database_object(database, object)->type = value;
}
/** Sets stack on game object. @param[in,out] database Game database. @param[in]
 * object Game object. @param[in,out] value Value to use. */

static inline void game_object_set_stack(GameDatabase *database, DbRef object,
                                         AttributeStack *value) {
  game_database_object(database, object)->stackhead = value;
}

/** Parses dbref. @param[in] s String or object to process. */

extern DbRef parse_dbref(const char *s);
/** Adds al. @param[in] object Game object. @param[in] attribute_number
 * Attribute number. */

extern void al_add(DbRef object, int attribute_number);
/** Executes al delete. @param[in] object Game object. @param[in]
 * attribute_number Attribute number. */

extern void al_delete(DbRef object, int attribute_number);
/** Destroys al. @param[in] object Game object. */

extern void al_destroy(DbRef object);
/** Executes al store. */

extern void al_store(void);
/** Executes db grow. @param[in,out] database Game database. @param[in] newtop
 * Newtop. */

extern void db_grow(GameDatabase *database, DbRef newtop);
/** Releases db. @param[in,out] database Game database. */

extern void db_free(GameDatabase *database);
/** Sets object password. @param[in,out] database Game database. @param[in]
 * thing Thing. @param[in] s String or object to process. */

[[nodiscard]] bool object_password_set(GameDatabase *database, DbRef thing,
                                       const char *s);
/** Sets object name. @param[in,out] database Game database. @param[in] thing
 * Thing. @param[in] s String or object to process. */

void object_name_set(GameDatabase *database, DbRef thing, const char *s);
/** Executes game object name. @param[in] database Game database. @param[in]
 * thing Thing. */

const char *game_object_name(GameDatabase *database, DbRef thing);
/** Executes game object pure name. @param[in] database Game database.
 * @param[in] thing Thing. */

const char *game_object_pure_name(GameDatabase *database, DbRef thing);
/** Returns an object's description, or nullptr when unset. */

const char *game_object_description(GameDatabase *database, DbRef object);
/** Sets or clears an object's description. */

void game_object_description_set(GameDatabase *database, DbRef object,
                                 const char *description);
/** Returns an object's internal description, or nullptr when unset. */

const char *game_object_internal_description(GameDatabase *database,
                                             DbRef object);
/** Sets or clears an object's internal description. */

void game_object_internal_description_set(GameDatabase *database, DbRef object,
                                          const char *description);
/** Executes game object lua parent. @param[in,out] database Game database.
 * @param[in] object Game object. */

const char *game_object_lua_parent(GameDatabase *database, DbRef object);
/** Sets game object lua parent. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in] path Filesystem path. */

bool game_object_lua_parent_set(GameDatabase *database, DbRef object,
                                const char *path);
/** Executes init min db. */

void init_min_db(void);
/** Pushes attribute stack. */

void attribute_stack_push(void);
/** Pops attribute stack. */

void attribute_stack_pop(void);
/** Executes init gdbm db. @param[in,out] path Filesystem path. */

int init_gdbm_db(char *path);
typedef struct GameObjectOwnedStateCopyRequest {
  EvaluationContext *evaluation;
  DbRef source;
  DbRef destination;
} GameObjectOwnedStateCopyRequest;

/** Copies core object-owned state. @param[in] request Request. */

void game_object_owned_state_copy(
    const GameObjectOwnedStateCopyRequest *request);
/** Releases core object-owned state. */
void game_object_owned_state_clear(GameDatabase *database, DbRef thing);
/** Executes toast player. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. */

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
