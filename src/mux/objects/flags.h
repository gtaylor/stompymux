/* flags.h - object flags */

#pragma once

#include "mux/objects/db.h"
#include "mux/support/hash_table.h"

typedef enum ObjectFlag {
  OBJECT_FLAG_NONE,
  OBJECT_FLAG_ANSI,
  OBJECT_FLAG_ANSIMAP,
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
  OBJECT_FLAG_QUIET,
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

typedef struct FlagEntry {
  /** Player-facing name used for matching, display, and configuration. */
  const char *flagname;
  /** In-memory field selected by this entry. */
  ObjectFlag id;
  /** Single-character abbreviation shown in compact flag displays. */
  char flaglett;
  /** Validates and applies a requested change to this flag. */
  bool (*handler)(EvaluationContext *, DbRef, DbRef, ObjectFlag, bool);
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
extern ObjectEntry object_types[8];

constexpr int OF_CONTENTS = 0x0001;
constexpr int OF_LOCATION = 0x0002;
constexpr int OF_EXITS = 0x0004;
constexpr int OF_HOME = 0x0008;
constexpr int OF_DROPTO = 0x0010;
constexpr int OF_SIBLINGS = 0x0040;

typedef struct WorldIndexes WorldIndexes;
typedef struct WorldContext WorldContext;

void init_flagtab(WorldIndexes *indexes);
void display_flagtab(EvaluationContext *, DbRef);
void flag_set(EvaluationContext *, WorldIndexes *indexes, DbRef, DbRef, char *,
              int);
char *flag_description(GameDatabase *, DbRef, DbRef);
FlagEntry *find_flag(WorldIndexes *, DbRef, char *);
char *decode_flags(GameDatabase *, DbRef, int, const ObjectFlagSet *);
bool has_flag(WorldContext *world, DbRef, DbRef, char *);
char *unparse_object(GameDatabase *database, EvaluationContext *evaluation,
                     DbRef player, DbRef target);
char *unparse_object_numonly(GameDatabase *database, DbRef object);
bool convert_flags(EvaluationContext *, DbRef, char *, ObjectFlagSet *, long *);

bool game_object_has_flag(GameDatabase *, DbRef, ObjectFlag);
void game_object_set_flag(GameDatabase *, DbRef, ObjectFlag, bool);
void game_object_clear_flags(GameDatabase *, DbRef);
void game_object_flags_copy(GameDatabase *, DbRef, ObjectFlagSet *);
bool object_flag_set_has(const ObjectFlagSet *, ObjectFlag);
void object_flag_set_set(ObjectFlagSet *, ObjectFlag, bool);

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
  return (object_types[typeof_obj(database, x)].flags & OF_LOCATION) != 0;
}
static inline bool has_contents(GameDatabase *database, DbRef x) {
  return (object_types[typeof_obj(database, x)].flags & OF_CONTENTS) != 0;
}
static inline bool has_exits(GameDatabase *database, DbRef x) {
  return (object_types[typeof_obj(database, x)].flags & OF_EXITS) != 0;
}
static inline bool has_siblings(GameDatabase *database, DbRef x) {
  return (object_types[typeof_obj(database, x)].flags & OF_SIBLINGS) != 0;
}
static inline bool has_home(GameDatabase *database, DbRef x) {
  return (object_types[typeof_obj(database, x)].flags & OF_HOME) != 0;
}
static inline bool has_dropto(GameDatabase *database, DbRef x) {
  return (object_types[typeof_obj(database, x)].flags & OF_DROPTO) != 0;
}

bool is_good_obj(GameDatabase *database, DbRef x);
#define OBJECT_FLAG_PREDICATE(name, id)                                        \
  static inline bool is_##name(GameDatabase *database, DbRef x) {              \
    return game_object_has_flag(database, x, id);                              \
  }
OBJECT_FLAG_PREDICATE(ansi, OBJECT_FLAG_ANSI)
OBJECT_FLAG_PREDICATE(ansimap, OBJECT_FLAG_ANSIMAP)
OBJECT_FLAG_PREDICATE(no_command, OBJECT_FLAG_NO_COMMAND)
OBJECT_FLAG_PREDICATE(transparent, OBJECT_FLAG_TRANSPARENT)
OBJECT_FLAG_PREDICATE(quiet, OBJECT_FLAG_QUIET)
OBJECT_FLAG_PREDICATE(halted, OBJECT_FLAG_HALTED)
OBJECT_FLAG_PREDICATE(going, OBJECT_FLAG_GOING)
OBJECT_FLAG_PREDICATE(monitor, OBJECT_FLAG_MONITOR)
OBJECT_FLAG_PREDICATE(audible, OBJECT_FLAG_AUDIBLE)
OBJECT_FLAG_PREDICATE(gagged, OBJECT_FLAG_GAGGED)
OBJECT_FLAG_PREDICATE(auditorium, OBJECT_FLAG_AUDITORIUM)
OBJECT_FLAG_PREDICATE(floating, OBJECT_FLAG_FLOATING)
OBJECT_FLAG_PREDICATE(light, OBJECT_FLAG_LIGHT)
OBJECT_FLAG_PREDICATE(xcode, OBJECT_FLAG_XCODE)
OBJECT_FLAG_PREDICATE(zombie, OBJECT_FLAG_ZOMBIE)
OBJECT_FLAG_PREDICATE(in_character, OBJECT_FLAG_IN_CHARACTER)
OBJECT_FLAG_PREDICATE(suspect, OBJECT_FLAG_SUSPECT)
OBJECT_FLAG_PREDICATE(hidden, OBJECT_FLAG_DARK)
#undef OBJECT_FLAG_PREDICATE

static inline bool is_wizard(GameDatabase *database, DbRef x) {
  return game_database_object(database, x)->has_wizard_flag;
}

static inline bool is_connected(GameDatabase *database, DbRef x) {
  return game_object_has_flag(database, x, OBJECT_FLAG_CONNECTED) &&
         is_player(database, x);
}
static inline bool is_alive(GameDatabase *database, DbRef x) {
  return is_player(database, x);
}
static inline bool is_dark(GameDatabase *database, DbRef x) {
  return game_object_has_flag(database, x, OBJECT_FLAG_DARK) &&
         (is_wizard(database, x) || !is_alive(database, x));
}

bool is_safe(GameDatabase *, const ServerConfiguration *, DbRef, DbRef);
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
  return is_wizard(database, player) && !is_wizard(database, target) &&
         !is_god(database, target);
}

void mark(GameDatabase *, DbRef);
void unmark(GameDatabase *, DbRef);
bool is_marked(GameDatabase *, DbRef);
void unmark_all(GameDatabase *);
bool can_link_exit(GameDatabase *, DbRef, DbRef);
bool is_linkable(GameDatabase *, DbRef, DbRef);
bool see_attr(EvaluationContext *, DbRef, DbRef, Attribute *, long);
bool see_attr_explicit(GameDatabase *, DbRef, DbRef, Attribute *, long);
bool set_attr(EvaluationContext *, DbRef, DbRef, Attribute *, long);
bool read_attr(EvaluationContext *, DbRef, DbRef, Attribute *, long);
bool write_attr(EvaluationContext *, DbRef, DbRef, Attribute *, long);

#define OBJECT_FLAG_MUTATOR(name, id)                                          \
  static inline void s_##name(GameDatabase *database, DbRef x) {               \
    game_object_set_flag(database, x, id, true);                               \
  }
OBJECT_FLAG_MUTATOR(halted, OBJECT_FLAG_HALTED)
OBJECT_FLAG_MUTATOR(going, OBJECT_FLAG_GOING)
OBJECT_FLAG_MUTATOR(connected, OBJECT_FLAG_CONNECTED)
OBJECT_FLAG_MUTATOR(xcode, OBJECT_FLAG_XCODE)
OBJECT_FLAG_MUTATOR(zombie, OBJECT_FLAG_ZOMBIE)
OBJECT_FLAG_MUTATOR(in_character, OBJECT_FLAG_IN_CHARACTER)
OBJECT_FLAG_MUTATOR(dark, OBJECT_FLAG_DARK)
#undef OBJECT_FLAG_MUTATOR
static inline void c_xcode(GameDatabase *database, DbRef x) {
  game_object_set_flag(database, x, OBJECT_FLAG_XCODE, false);
}
static inline void c_connected(GameDatabase *database, DbRef x) {
  game_object_set_flag(database, x, OBJECT_FLAG_CONNECTED, false);
}
static inline char *unparse_flags(GameDatabase *database, DbRef p, DbRef t) {
  ObjectFlagSet flags = {0};
  game_object_flags_copy(database, t, &flags);
  return decode_flags(database, p, typeof_obj(database, t), &flags);
}
