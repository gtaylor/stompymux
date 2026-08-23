/** @file
 * object flags.
 */
#pragma once

#include <stddef.h>

#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/world/world_context.h"

typedef enum ObjectFlag : int {
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

/** Executes object type entry. @param[in] type Type. */

const ObjectEntry *object_type_entry(int type);

constexpr int OF_CONTENTS = 0x0001;
constexpr int OF_LOCATION = 0x0002;
constexpr int OF_EXITS = 0x0004;
constexpr int OF_HOME = 0x0008;
constexpr int OF_DROPTO = 0x0010;
constexpr int OF_SIBLINGS = 0x0040;

typedef struct WorldIndexes WorldIndexes;
typedef struct WorldContext WorldContext;

/** Executes init flagtab. @param[in,out] indexes Indexes. */

void init_flagtab(WorldIndexes *indexes);
/** Returns the number of canonical object flag registry entries. */

size_t object_flag_entry_count(void);
/** Returns the canonical object flag entry at a valid zero-based index.
 * @param[in] index Zero-based registry index smaller than
 * object_flag_entry_count(). */

const FlagEntry *object_flag_entry_at(size_t index);
/** Executes display flagtab. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. */

void display_flagtab(EvaluationContext *evaluation, DbRef player);
/** Sets flag. @param[in,out] evaluation Expression evaluation context.
 * @param[in,out] indexes Indexes. @param[in] target Target object or value.
 * @param[in] player Player object. @param[in,out] name Name to use. @param[in]
 * key Lookup key or command flags. */

void flag_set(EvaluationContext *evaluation, WorldIndexes *indexes,
              DbRef target, DbRef player, char *name, int key);
/** Executes flag description. @param[in,out] database Game database. @param[in]
 * target Target object or value. */

OwnedText flag_description(GameDatabase *database, DbRef target);
/** Executes flags description. @param[in,out] database Game database.
 * @param[in] target Target object or value. */

OwnedText flags_description(GameDatabase *database, DbRef target);
/** Finds find flag. @param[in] indexes Indexes. @param[in] thing Thing.
 * @param[in] flagname Flagname. */

const FlagEntry *find_flag(WorldIndexes *indexes, DbRef thing,
                           const char *flagname);
/** Adds flag alias. @param[in,out] indexes Indexes. @param[in] alias Alias.
 * @param[in] flag Flag. */

bool flag_alias_add(WorldIndexes *indexes, const char *alias,
                    const FlagEntry *flag);
/** Executes decode flags. @param[in] request Request. */

OwnedText decode_flags(const DecodeFlagsRequest *request);
/** Executes unparse object. @param[in] database Game database. @param[in]
 * evaluation Expression evaluation context. @param[in] player Player object.
 * @param[in] target Target object or value. */

OwnedText unparse_object(GameDatabase *database, EvaluationContext *evaluation,
                         DbRef player, DbRef target);
/** Executes unparse object numonly. @param[in] database Game database.
 * @param[in] target Target object or value. */

OwnedText unparse_object_numonly(GameDatabase *database, DbRef target);
/** Executes convert flags. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. @param[in,out] list List.
 * @param[in,out] flags Flags. @param[in,out] type Type. */

bool convert_flags(EvaluationContext *evaluation, DbRef player, char *list,
                   ObjectFlagSet *flags, long *type);

/** Executes game object has flag. @param[in] request Request. */

bool game_object_has_flag(const ObjectFlagRequest *request);
/** Sets flag on game object. @param[in] request Request. */

void game_object_set_flag(const ObjectFlagChangeRequest *request);
/** Executes game object clear flags. @param[in,out] database Game database.
 * @param[in] object Game object. */

void game_object_clear_flags(GameDatabase *database, DbRef object);
/** Executes game object flags copy. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in,out] flags Flags. */

void game_object_flags_copy(GameDatabase *database, DbRef object,
                            ObjectFlagSet *flags);
/** Sets has on object flag. @param[in] flags Flags. @param[in] flag Flag. */

bool object_flag_set_has(const ObjectFlagSet *flags, ObjectFlag flag);
/** Sets object flag set. @param[in,out] flags Flags. @param[in] flag Flag.
 * @param[in] value Value to use. */

void object_flag_set_set(ObjectFlagSet *flags, ObjectFlag flag, bool value);

constexpr DbRef GOD = 1;

/** Executes typeof obj. @param[in,out] database Game database. @param[in] x X.
 */

static inline int typeof_obj(GameDatabase *database, DbRef x) {
  return (int)(unsigned)game_database_object(database, x)->type;
}
/** Reports whether is god. @param[in] database Game database. @param[in] x X.
 */

static inline bool is_god(GameDatabase *database [[maybe_unused]], DbRef x) {
  return x == GOD;
}
/** Reports whether is player. @param[in] database Game database. @param[in] x
 * X. */

static inline bool is_player(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_PLAYER;
}
/** Reports whether is room. @param[in] database Game database. @param[in] x X.
 */

static inline bool is_room(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_ROOM;
}
/** Reports whether is exit. @param[in] database Game database. @param[in] x X.
 */

static inline bool is_exit(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_EXIT;
}
/** Reports whether is thing. @param[in] database Game database. @param[in] x X.
 */

static inline bool is_thing(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == OBJECT_TYPE_THING;
}
/** Reports whether has location. @param[in] database Game database. @param[in]
 * x X. */

static inline bool has_location(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_LOCATION) != 0;
}
/** Reports whether has contents. @param[in] database Game database. @param[in]
 * x X. */

static inline bool has_contents(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_CONTENTS) != 0;
}
/** Reports whether has exits. @param[in] database Game database. @param[in] x
 * X. */

static inline bool has_exits(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_EXITS) != 0;
}
/** Reports whether has siblings. @param[in] database Game database. @param[in]
 * x X. */

static inline bool has_siblings(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_SIBLINGS) != 0;
}
/** Reports whether has home. @param[in] database Game database. @param[in] x X.
 */

static inline bool has_home(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_HOME) != 0;
}
/** Reports whether has dropto. @param[in] database Game database. @param[in] x
 * X. */

static inline bool has_dropto(GameDatabase *database, DbRef x) {
  return (object_type_entry(typeof_obj(database, x))->flags & OF_DROPTO) != 0;
}

/** Reports whether is good obj. @param[in] database Game database. @param[in] x
 * X. */

bool is_good_obj(GameDatabase *database, DbRef x);
/** Executes object has flag. @param[in] object Game object. @param[in,out]
 * database Game database. @param[in] flag Flag. */

static inline bool object_has_flag(DbRef object, GameDatabase *database,
                                   ObjectFlag flag) {
  return game_object_has_flag(&(ObjectFlagRequest){
      .database = database, .object = object, .flag = flag});
}
/** Reports whether is ansi. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_ansi(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_ANSI);
}
/** Reports whether is no command. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_no_command(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_NO_COMMAND);
}
/** Reports whether is transparent. @param[in] database Game database.
 * @param[in] object Game object. */

static inline bool is_transparent(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_TRANSPARENT);
}
/** Reports whether is halted. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_halted(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_HALTED);
}
/** Reports whether is going. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_going(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_GOING);
}
/** Reports whether is monitor. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_monitor(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_MONITOR);
}
/** Reports whether is audible. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_audible(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_AUDIBLE);
}
/** Reports whether is gagged. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_gagged(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_GAGGED);
}
/** Reports whether is auditorium. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_auditorium(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_AUDITORIUM);
}
/** Reports whether is floating. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_floating(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_FLOATING);
}
/** Reports whether is light. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_light(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_LIGHT);
}
/** Reports whether is xcode. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_xcode(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_XCODE);
}
/** Reports whether is zombie. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_zombie(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_ZOMBIE);
}
/** Reports whether is in character. @param[in] database Game database.
 * @param[in] object Game object. */

static inline bool is_in_character(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_IN_CHARACTER);
}
/** Reports whether is suspect. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_suspect(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_SUSPECT);
}
/** Reports whether is hidden. @param[in] database Game database. @param[in]
 * object Game object. */

static inline bool is_hidden(GameDatabase *database, DbRef object) {
  return object_has_flag(object, database, OBJECT_FLAG_DARK);
}

/** Reports whether is wizard. @param[in] database Game database. @param[in] x
 * X. */

static inline bool is_wizard(GameDatabase *database, DbRef x) {
  return game_database_object(database, x)->has_wizard_flag;
}

/** Reports whether is connected. @param[in] database Game database. @param[in]
 * x X. */

static inline bool is_connected(GameDatabase *database, DbRef x) {
  return (game_object_has_flag(
              &(ObjectFlagRequest){.database = database,
                                   .object = x,
                                   .flag = OBJECT_FLAG_CONNECTED}) &&
          is_player(database, x)) != 0;
}
/** Reports whether is alive. @param[in] database Game database. @param[in] x X.
 */

static inline bool is_alive(GameDatabase *database, DbRef x) {
  return is_player(database, x);
}
/** Reports whether is dark. @param[in] database Game database. @param[in] x X.
 */

static inline bool is_dark(GameDatabase *database, DbRef x) {
  return (game_object_has_flag(&(ObjectFlagRequest){
              .database = database, .object = x, .flag = OBJECT_FLAG_DARK}) &&
          (is_wizard(database, x) || !is_alive(database, x))) != 0;
}

/** Reports whether is safe. @param[in] database Game database. @param[in]
 * object Game object. */

bool is_safe(GameDatabase *database, DbRef object);
/** Reports whether is examinable. @param[in] database Game database. @param[in]
 * player Player object. @param[in] target Target object or value. */

static inline bool is_examinable(GameDatabase *database, DbRef player,
                                 DbRef target) {
  return (target >= 0 && target < database->top &&
          typeof_obj(database, target) != OBJECT_TYPE_GARBAGE &&
          (is_god(database, player) || is_wizard(database, player))) != 0;
}
/** Reports whether is controls. @param[in] database Game database. @param[in]
 * player Player object. @param[in] target Target object or value. */

static inline bool is_controls(GameDatabase *database, DbRef player,
                               DbRef target) {
  if (target < 0 || target >= database->top ||
      typeof_obj(database, target) == OBJECT_TYPE_GARBAGE)
    return false;
  if (is_god(database, player))
    return true;
  if (player == target)
    return is_wizard(database, player);
  return (is_wizard(database, player) && !is_wizard(database, target) &&
          !is_god(database, target)) != 0;
}

/** Executes mark. @param[in,out] database Game database. @param[in] x X. */

void mark(GameDatabase *database, DbRef x);
/** Executes unmark. @param[in,out] database Game database. @param[in] x X. */

void unmark(GameDatabase *database, DbRef x);
/** Reports whether is marked. @param[in] database Game database. @param[in] x
 * X. */

bool is_marked(GameDatabase *database, DbRef x);
/** Executes unmark all. @param[in,out] database Game database. */

void unmark_all(GameDatabase *database);
/** Reports whether can link exit. @param[in] database Game database. @param[in]
 * player Player object. @param[in] target Target object or value. */

bool can_link_exit(GameDatabase *database, DbRef player, DbRef target);
/** Reports whether is linkable. @param[in] database Game database. @param[in]
 * player Player object. @param[in] target Target object or value. */

bool is_linkable(GameDatabase *database, DbRef player, DbRef target);

/** Executes object flag enable. @param[in] object Game object. @param[in,out]
 * database Game database. @param[in] flag Flag. */

static inline void object_flag_enable(DbRef object, GameDatabase *database,
                                      ObjectFlag flag) {
  game_object_set_flag(&(ObjectFlagChangeRequest){
      .database = database, .object = object, .flag = flag, .value = true});
}
/** Sets the halted flag. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline void s_halted(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_HALTED);
}
/** Sets the going flag. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline void s_going(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_GOING);
}
/** Sets the connected flag. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline void s_connected(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_CONNECTED);
}
/** Sets the xcode flag. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline void s_xcode(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_XCODE);
}
/** Sets the zombie flag. @param[in,out] database Game database. @param[in]
 * object Game object. */

static inline void s_zombie(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_ZOMBIE);
}
/** Sets the in character flag. @param[in,out] database Game database.
 * @param[in] object Game object. */

static inline void s_in_character(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_IN_CHARACTER);
}
/** Sets the dark flag. @param[in,out] database Game database. @param[in] object
 * Game object. */

static inline void s_dark(GameDatabase *database, DbRef object) {
  object_flag_enable(object, database, OBJECT_FLAG_DARK);
}
/** Clears the xcode flag. @param[in,out] database Game database. @param[in] x
 * X. */

static inline void c_xcode(GameDatabase *database, DbRef x) {
  game_object_set_flag(&(ObjectFlagChangeRequest){
      .database = database, .object = x, .flag = OBJECT_FLAG_XCODE});
}
/** Clears the connected flag. @param[in,out] database Game database. @param[in]
 * x X. */

static inline void c_connected(GameDatabase *database, DbRef x) {
  game_object_set_flag(&(ObjectFlagChangeRequest){
      .database = database, .object = x, .flag = OBJECT_FLAG_CONNECTED});
}
/** Executes unparse flags. @param[in] database Game database. @param[in] p P.
 * @param[in] t T. */

static inline OwnedText unparse_flags(GameDatabase *database, DbRef p,
                                      DbRef t) {
  ObjectFlagSet flags = {};
  game_object_flags_copy(database, t, &flags);
  return decode_flags(
      &(DecodeFlagsRequest){.database = database,
                            .player = p,
                            .object_type = typeof_obj(database, t),
                            .flags = &flags});
}
