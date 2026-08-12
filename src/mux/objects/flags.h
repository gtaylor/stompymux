/* flags.h - object flags */

#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"
#include "mux/support/hash_table.h"
#include "mux/world/world_context.h"

typedef enum ObjectFlag {
  OBJECT_FLAG_NONE,
  OBJECT_FLAG_ANSI,
  OBJECT_FLAG_AUDIBLE,
  OBJECT_FLAG_AUDITORIUM,
  OBJECT_FLAG_BLIND,
  OBJECT_FLAG_CONNECTED,
  OBJECT_FLAG_DARK,
  OBJECT_FLAG_FLOATING,
  OBJECT_FLAG_GAGGED,
  OBJECT_FLAG_GOING,
  OBJECT_FLAG_HALTED,
  OBJECT_FLAG_IN_CHARACTER,
  OBJECT_FLAG_LIGHT,
  OBJECT_FLAG_MONITOR,
  OBJECT_FLAG_NO_COMMAND,
  OBJECT_FLAG_SAFE,
  OBJECT_FLAG_SUSPECT,
  OBJECT_FLAG_TRANSPARENT,
  OBJECT_FLAG_WIZARD,
  OBJECT_FLAG_XCODE,
  OBJECT_FLAG_ZOMBIE,
  OBJECT_FLAG_COUNT,
} ObjectFlag;

typedef struct ObjectFlagSet ObjectFlagSet;
struct ObjectFlagSet {
  bool values[OBJECT_FLAG_COUNT];
};

typedef struct ObjectFlagRequest {
  GameDatabase *database;
  DbRef object;
  ObjectFlag flag;
} ObjectFlagRequest;

typedef struct ObjectFlagChangeRequest {
  GameDatabase *database;
  DbRef object;
  ObjectFlag flag;
  bool value;
} ObjectFlagChangeRequest;

typedef struct FlagChangeRequest {
  EvaluationContext *evaluation;
  DbRef target;
  DbRef player;
  ObjectFlag flag;
  bool clear;
} FlagChangeRequest;

typedef struct DecodeFlagsRequest {
  GameDatabase *database;
  DbRef player;
  int object_type;
  const ObjectFlagSet *flags;
} DecodeFlagsRequest;

typedef struct FlagEntry {
  /** Player-facing name used for matching, display, and configuration. */
  const char *flagname;
  /** In-memory field selected by this entry. */
  ObjectFlag id;
  /** Single-character abbreviation shown in compact flag displays. */
  char flaglett;
  /** Validates and applies a requested change to this flag. */
  bool (*handler)(const FlagChangeRequest *request);
} FlagEntry;

typedef struct ObjectEntry {
  /** Player-facing name of this object type. */
  const char *name;
  /** Single-character abbreviation shown in compact object descriptions. */
  char lett;
  /** Access permission required to expose this object type. */
  int perm;
  /** Structural capabilities, composed from the OF_* constants below. */
  int flags;
} ObjectEntry;

const ObjectEntry *object_type_entry(int type);

constexpr int OF_CONTENTS = 0x0001;
constexpr int OF_LOCATION = 0x0002;
constexpr int OF_EXITS = 0x0004;
constexpr int OF_HOME = 0x0008;
constexpr int OF_DROPTO = 0x0010;
constexpr int OF_SIBLINGS = 0x0040;

typedef struct WorldIndexes WorldIndexes;
typedef struct WorldContext WorldContext;

void init_flagtab(WorldIndexes *indexes);
void display_flagtab(EvaluationContext * /*evaluation*/, DbRef /*player*/);
void flag_set(EvaluationContext * /*evaluation*/, WorldIndexes *indexes,
              DbRef /*target*/, DbRef /*player*/, char * /*name*/, int /*key*/);
char *flag_description(GameDatabase * /*database*/, DbRef target);
char *flags_description(GameDatabase * /*database*/, DbRef target);
FlagEntry *find_flag(WorldIndexes * /*indexes*/, DbRef /*thing*/,
                     char * /*flagname*/);
char *decode_flags(const DecodeFlagsRequest *request);
char *unparse_object(GameDatabase *database, EvaluationContext *evaluation,
                     DbRef player, DbRef target);
char *unparse_object_numonly(GameDatabase *database, DbRef target);
bool convert_flags(EvaluationContext * /*evaluation*/, DbRef /*player*/,
                   char * /*list*/, ObjectFlagSet * /*flags*/, long * /*type*/);

bool game_object_has_flag(const ObjectFlagRequest *request);
void game_object_set_flag(const ObjectFlagChangeRequest *request);
void game_object_clear_flags(GameDatabase * /*database*/, DbRef /*object*/);
void game_object_flags_copy(GameDatabase * /*database*/, DbRef /*object*/,
                            ObjectFlagSet * /*flags*/);
bool object_flag_set_has(const ObjectFlagSet * /*flags*/, ObjectFlag /*flag*/);
void object_flag_set_set(ObjectFlagSet * /*flags*/, ObjectFlag /*flag*/,
                         bool /*value*/);

constexpr DbRef GOD = 1;

static inline int typeof_obj(GameDatabase *database, DbRef x) {
  return (int)(unsigned)game_database_object(database, x)->type;
}
static inline bool is_god(GameDatabase *database, DbRef x) { return x == GOD; }
static inline bool is_player(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_PLAYER;
}
static inline bool is_room(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_ROOM;
}
static inline bool is_exit(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_EXIT;
}
static inline bool is_thing(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_THING;
}
static inline bool has_location(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_LOCATION) != 0;
}
static inline bool has_contents(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_CONTENTS) != 0;
}
static inline bool has_exits(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_EXITS) != 0;
}
static inline bool has_siblings(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_SIBLINGS) != 0;
}
static inline bool has_home(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_HOME) != 0;
}
static inline bool has_dropto(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_DROPTO) != 0;
}

bool is_good_obj(GameDatabase *database, DbRef x);
static inline bool object_has_flag(DbRef object, GameDatabase *database,
                                   ObjectFlag flag) {
  return game_object_has_flag(&(ObjectFlagRequest){
      .database = database, .object = object, .flag = flag});
}
static inline bool is_ansi(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_ANSI);
}
static inline bool is_no_command(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_NO_COMMAND);
}
static inline bool is_transparent(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_TRANSPARENT);
}
static inline bool is_halted(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_HALTED);
}
static inline bool is_going(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_GOING);
}
static inline bool is_monitor(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_MONITOR);
}
static inline bool is_audible(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_AUDIBLE);
}
static inline bool is_gagged(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_GAGGED);
}
static inline bool is_auditorium(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_AUDITORIUM);
}
static inline bool is_floating(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_FLOATING);
}
static inline bool is_light(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_LIGHT);
}
static inline bool is_xcode(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_XCODE);
}
static inline bool is_zombie(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_ZOMBIE);
}
static inline bool is_in_character(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_IN_CHARACTER);
}
static inline bool is_suspect(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_SUSPECT);
}
static inline bool is_hidden(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_DARK);
}

static inline bool is_wizard(GameDatabase *database, DbRef x) {
  return game_database_object(database, x)->has_wizard_flag;
}

static inline bool is_connected(GameDatabase *database, DbRef x) {
  return game_object_has_flag(
             &(ObjectFlagRequest){.database = database,
                                  .object = x,
                                  .flag = OBJECT_FLAG_CONNECTED}) &&
         is_player(database, x);
}
static inline bool is_alive(GameDatabase *database, DbRef x) {
  return is_player(database, x);
}
static inline bool is_dark(GameDatabase *database, DbRef x) {
  return game_object_has_flag(&(ObjectFlagRequest){
             .database = database, .object = x, .flag = OBJECT_FLAG_DARK}) &&
         (is_wizard(database, x) || !is_alive(database, x));
}

bool is_safe(GameDatabase * /*database*/, DbRef object);
static inline bool is_examinable(GameDatabase *database, DbRef player,
                                 DbRef target) {
  return target >= 0 && target < database->top &&
         typeof_obj(database, target) != OBJECT_TYPE_GARBAGE &&
         (is_god(database, player) || is_wizard(database, player));
}
static inline bool is_controls(GameDatabase *database, DbRef player,
                               DbRef target) {
  if (target < 0 || target >= database->top ||
      typeof_obj(database, target) == OBJECT_TYPE_GARBAGE)
    return false;
  if (is_god(database, player))
    return true;
  if (player == target)
    return is_wizard(database, player);
  return is_wizard(database, player) && !is_wizard(database, target) &&
         !is_god(database, target);
}

void mark(GameDatabase * /*database*/, DbRef /*x*/);
void unmark(GameDatabase * /*database*/, DbRef /*x*/);
bool is_marked(GameDatabase * /*database*/, DbRef /*x*/);
void unmark_all(GameDatabase * /*database*/);
bool can_link_exit(GameDatabase * /*database*/, DbRef /*player*/,
                   DbRef /*target*/);
bool is_linkable(GameDatabase * /*database*/, DbRef /*player*/,
                 DbRef /*target*/);

static inline void object_flag_enable(DbRef object, GameDatabase *database,
                                      ObjectFlag flag) {
  game_object_set_flag(&(ObjectFlagChangeRequest){
      .database = database, .object = object, .flag = flag, .value = true});
}
static inline void s_halted(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_HALTED);
}
static inline void s_going(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_GOING);
}
static inline void s_connected(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_CONNECTED);
}
static inline void s_xcode(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_XCODE);
}
static inline void s_zombie(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_ZOMBIE);
}
static inline void s_in_character(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_IN_CHARACTER);
}
static inline void s_dark(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_DARK);
}
static inline void c_xcode(GameDatabase *database, DbRef x) {
  game_object_set_flag(&(ObjectFlagChangeRequest){
      .database = database, .object = x, .flag = OBJECT_FLAG_XCODE});
}
static inline void c_connected(GameDatabase *database, DbRef x) {
  game_object_set_flag(&(ObjectFlagChangeRequest){
      .database = database, .object = x, .flag = OBJECT_FLAG_CONNECTED});
}
static inline char *unparse_flags(GameDatabase *database, DbRef p, DbRef t) {
  ObjectFlagSet flags = {0};
  game_object_flags_copy(database, t, &flags);
  return decode_flags(
      &(DecodeFlagsRequest){.database = database,
                            .player = p,
                            .object_type = typeof_obj(database, t),
                            .flags = &flags});
}
