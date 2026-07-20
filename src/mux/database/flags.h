/* flags.h - object flags */

#pragma once

#include "mux/database/db.h"
#include "mux/support/hash_table.h"

constexpr int FLAG_WORD2 = 0x1; /* 2nd word of flags. */
constexpr int FLAG_WORD3 = 0x2; /* 3rd word of flags. */

/* Object types */
constexpr int TYPE_ROOM = 0x0;
constexpr int TYPE_THING = 0x1;
constexpr int TYPE_EXIT = 0x2;
constexpr int TYPE_PLAYER = 0x3;

/* Empty */
constexpr int TYPE_GARBAGE = 0x5;
constexpr int NOTYPE = 0x7;
constexpr int TYPE_MASK = 0x7;

/* First word of flags */
constexpr int SEETHRU = 0x00000008; /* Can see through to the other side */
constexpr int WIZARD = 0x00000010;  /* gets automatic control */
/* 0x00000020 is reserved for the removed LINK_OK flag. */
constexpr int DARK = 0x00000040; /* Don't show contents or presence */
/* 0x00000080 is reserved for the removed JUMP_OK flag. */
constexpr int STICKY = 0x00000100; /* Object goes home when dropped */
/* 0x00000200 is reserved for the removed DESTROY_OK flag. */
/* 0x00000400 is reserved for the removed HAVEN flag. */
constexpr int QUIET = 0x00000800;   /* Prevent 'feelgood' messages */
constexpr int HALT = 0x00001000;    /* object cannot perform actions */
constexpr int TRACE = 0x00002000;   /* Generate evaluation trace output */
constexpr int GOING = 0x00004000;   /* object is available for recycling */
constexpr int MONITOR = 0x00008000; /* Process ^x:action listens on obj? */
constexpr int MYOPIC = 0x00010000;  /* See things as nonowner/nonwizard */
constexpr int PUPPET = 0x00020000;  /* Relays ALL messages to owner */
/* 0x00040000 is reserved for the removed CHOWN_OK flag. */
constexpr int ENTER_OK = 0x00080000; /* Object may be ENTERed */
/* 0x00100000 is reserved for the removed VISUAL flag. */
/* 0x00200000 is reserved for the removed IMMORTAL flag. */
/* 0x00400000 is reserved for the removed HAS_STARTUP flag. */
constexpr int OPAQUE = 0x00800000;  /* Can't see inside */
constexpr int VERBOSE = 0x01000000; /* Tells owner everything it does. */
constexpr int INHERIT = 0x02000000; /* Gets owner's privs. (i.e. Wiz) */
constexpr int NOSPOOF = 0x04000000; /* Report originator of all actions. */
constexpr int ROBOT = 0x08000000;   /* Player is a ROBOT */
constexpr int SAFE = 0x10000000;    /* Need /override to @destroy */
/* 0x20000000 is reserved for the removed ROYALTY flag. */
constexpr int HEARTHRU = 0x40000000; /* Can hear out of this obj or exit */
/* 0x80000000 is reserved for the removed TERSE flag. */

/* Second word of flags */
constexpr int KEY = 0x00000001; /* No puppets */
/* 0x00000002 is reserved for the removed ABODE flag. */
constexpr int FLOATING = 0x00000004;   /* Inhibit Floating room.. msgs */
constexpr int UNFINDABLE = 0x00000008; /* Cant loc() from afar */
/* 0x00000010 is reserved for the removed PARENT_OK flag. */
constexpr int LIGHT = 0x00000020;       /* Visible in dark places */
constexpr int HAS_LISTEN = 0x00000040;  /* Internal: LISTEN attr set */
constexpr int HAS_FWDLIST = 0x00000080; /* Internal: FORWARDLIST attr set */
constexpr int AUDITORIUM = 0x00000100;  /* Should we check the speech lock? */
constexpr int ANSI = 0x00000200;
/* 0x00000400 is reserved for the removed REGISTERED flag. */
constexpr int FIXED = 0x00000800;
/* 0x00001000 is reserved for the removed UNINSPECTED flag. */
constexpr int NO_COMMAND = 0x00002000;

constexpr int NOBLEED = 0x00008000;
/* 0x00010000 is reserved for the removed STAFF flag. */
/* 0x00020000 is reserved for the removed HAS_DAILY flag. */
constexpr int GAGGED = 0x00040000;
constexpr int HARDCODE = 0x00080000;
constexpr int IN_CHARACTER = 0x00100000;
constexpr int ANSIMAP = 0x00200000; /* Player uses ANSI maps */
/* 0x00400000 is reserved for the removed HAS_HOURLY flag. */
/* 0x00800000 is reserved for the removed MULTIOK flag. */

/* 0x01000000 is reserved for the removed VACATION flag. */
constexpr int BLIND = 0x04000000;  /* Something to support blind players! */
constexpr int ZOMBIE = 0x08000000; /* Hardcode object is a zombie */

constexpr int SUSPECT = 0x10000000; /* Report some activities to wizards */
/* 0x20000000 is reserved for the removed COMPRESS flag. */
constexpr int CONNECTED = 0x40000000; /* Player is connected */
/* 0x80000000 is reserved for the removed SLAVE flag. */

/* ---------------------------------------------------------------------------
 * FLAGENT: Information about object flags.
 */
typedef struct flag_entry {
  const char *flagname; /* Name of the flag */
  int flagvalue;        /* Which bit in the object is the flag */
  char flaglett;        /* Flag letter for listing */
  int flagflag;         /* Ctrl flags for this flag (recursive? :-) */
  int listperm;         /* Who sees this flag when set */
  int (*handler)(EvaluationContext *, DbRef, DbRef, Flag, int,
                 int); /* Handler for setting/clearing this flag */
} FLAGENT;

/* ---------------------------------------------------------------------------
 * OBJENT: Fundamental object types
 */
typedef struct object_entry {
  const char *name;
  char lett;
  int perm;
  int flags;
} OBJENT;
extern OBJENT object_types[8];

constexpr int OF_CONTENTS = 0x0001; /* Object has contents. */
constexpr int OF_LOCATION =
    0x0002; /* Object has a location: game_object_location(database, ) */
constexpr int OF_EXITS = 0x0004;    /* Object has exits. */
constexpr int OF_HOME = 0x0008;     /* Object has a home. */
constexpr int OF_DROPTO = 0x0010;   /* Object has a dropto. */
constexpr int OF_OWNER = 0x0020;    /* Object can own other objects */
constexpr int OF_SIBLINGS = 0x0040; /* Object has siblings. */

typedef struct flagset {
  Flag word1;
  Flag word2;
  Flag word3;
} FLAGSET;

typedef struct WorldIndexes WorldIndexes;
typedef struct WorldContext WorldContext;

extern void init_flagtab(WorldIndexes *indexes);
extern void display_flagtab(EvaluationContext *, DbRef);
extern void flag_set(EvaluationContext *, WorldIndexes *indexes, DbRef, DbRef,
                     char *, int);
extern char *flag_description(GameDatabase *, DbRef, DbRef);
extern FLAGENT *find_flag(WorldIndexes *indexes, DbRef, char *);
extern char *decode_flags(GameDatabase *, DbRef, Flag, long, long);
extern int has_flag(WorldContext *world, DbRef, DbRef, char *);
extern char *unparse_object(GameDatabase *database,
                            EvaluationContext *evaluation, DbRef player,
                            DbRef target, int obey_myopic);
extern char *unparse_object_numonly(GameDatabase *database, DbRef object);
extern int convert_flags(EvaluationContext *, DbRef, char *, FLAGSET *, Flag *);

constexpr DbRef GOD = 1;

/* ---------------------- Object Permission/Attribute Functions */
/* is_flag_set(database, X,T,F) - Is X of type T and have flag F set? */
/* typeof(X)         - What object type is X */
/* is_god(database, X)            - Is X player #1 */
/* is_robot_player(database, X)          - Is X a robot player */
/* is_wizard(database, X)         - Does X have wizard privs */
/* is_alive(database, X)          - Is X a player or a puppet */
/* is_dark(database, X)           - Is X dark */
/* is_floating(database, X)       - Prevent 'disconnected room' msgs for room X
 */
/* is_quiet(database, X)       - Should 'Set.' messages et al from X be disabled
 */
/* is_verbose(database, X)        - Should owner receive all commands executed?
 */
/* is_trace(database, X)          - Should owner receive eval trace output? */
/* is_halted(database, X)         - Is X halted (not allowed to run commands)?
 */
/* is_suspect(database, X)        - Is X someone the wizzes should keep an eye
 * on */
/* is_safe(X,P)         - Does P need the /OVERRIDE switch to @destroy X? */
/* is_monitor(database, X)        - Should we check for ^xxx:xxx listens on
 * player? */
/* is_myopic(database, X)         - Should things as if we were nonowner/nonwiz
 */
/* is_audible(database, X)        - Should X forward messages? */
/* is_findable(database, X)       - Can @whereis find X */
/* is_hideout(database, X)        - Is @whereis blocked for X */
/* has_location(database, X)   - Is X something with a location (ie plyr or obj)
 */
/* has_home(database, X)       - Is X something with a home (ie plyr or obj) */
/* has_contents(database, X)   - Is X something with contents (ie plyr/obj/room)
 */
/* is_good_obj(X)       - Is X inside the DB and have a valid type? */
/* is_good_owner(database, X)  - Is X a good owner value? */
/* is_going(database, X)          - Is X marked GOING? */
/* is_inherits(database, X)       - Does X inherit the privs of its owner */
/* is_examinable(P,X)   - Can P look at attribs of X */
/* is_myopic_exam(P,X)  - Can P look at attribs of X (obeys MYOPIC) */
/* is_controls(P,X)     - Can P force X to do something */
/* can_link_exit(P,X) - Can P link from exit X */
/* is_linkable(P,X)     - Can P link to X */
/* mark(x)           - Set marked flag on X */
/* unmark(x)         - Clear marked flag on X */
/* is_marked(x)         - Check marked flag on X */
/* is_hardcode(database, x)       - Check hardcode flag on X */
/* is_in_character(database, x)   - Whether or not mecha's IC */
/* see_attr(P,X.A,O,F)  - Can P see text attr A on X if attr has owner O */
/* set_attr(P,X,A,F)    - Can P set/change text attr A (with flags F) on X */
/* read_attr(P,X,A,O,F) - Can P see attr A on X if attr has owner O */
/* write_attr(P,X,A,F)  - Can P set/change attr A (with flags F) on X */

static inline int typeof_obj(GameDatabase *database, DbRef x) {
  return game_object_flags(database, x) & TYPE_MASK;
}
static inline bool is_flag_set(GameDatabase *database, DbRef thing, int type,
                               int flag) {
  return typeof_obj(database, thing) == type &&
         (game_object_flags(database, thing) & flag);
}
static inline bool is_god(GameDatabase *database, DbRef x) { return x == GOD; }
static inline bool is_robot(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & ROBOT) != 0;
}
static inline bool is_player(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == TYPE_PLAYER;
}
static inline bool is_room(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == TYPE_ROOM;
}
static inline bool is_exit(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == TYPE_EXIT;
}
static inline bool is_thing(GameDatabase *database, DbRef x) {
  return typeof_obj(database, x) == TYPE_THING;
}
static inline bool is_owns_others(GameDatabase *database, DbRef x) {
  return (object_types[typeof_obj(database, x)].flags & OF_OWNER) != 0;
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
// Defined in flags.c, where the database implementation is visible.
bool is_good_obj(GameDatabase *database, DbRef x);
static inline bool is_fixed(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & FIXED) != 0;
}
static inline bool is_ansi(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & ANSI) != 0;
}
static inline bool is_ansimap(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & ANSIMAP) != 0;
}
static inline bool is_no_command(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & NO_COMMAND) != 0;
}
static inline bool is_transparent(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & SEETHRU) != 0;
}
static inline bool is_sticky(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & STICKY) != 0;
}
static inline bool is_quiet(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & QUIET) != 0;
}
static inline bool is_halted(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & HALT) != 0;
}
static inline bool is_trace(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & TRACE) != 0;
}
static inline bool is_going(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & GOING) != 0;
}
static inline bool is_monitor(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & MONITOR) != 0;
}
static inline bool is_myopic(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & MYOPIC) != 0;
}
static inline bool is_puppet(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & PUPPET) != 0;
}
static inline bool is_opaque(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & OPAQUE) != 0;
}
static inline bool is_verbose(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & VERBOSE) != 0;
}
static inline bool is_nospoof(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & NOSPOOF) != 0;
}
static inline bool is_audible(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & HEARTHRU) != 0;
}
static inline bool is_gagged(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & GAGGED) != 0;
}
static inline bool has_key_flag(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & KEY) != 0;
}
static inline bool is_auditorium(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & AUDITORIUM) != 0;
}
static inline bool is_floating(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & FLOATING) != 0;
}
static inline bool is_findable(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & UNFINDABLE) == 0;
}
static inline bool is_hideout(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & UNFINDABLE) != 0;
}
static inline bool is_light(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & LIGHT) != 0;
}
static inline bool is_hardcode(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & HARDCODE) != 0;
}
static inline bool is_zombie(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & ZOMBIE) != 0;
}
static inline bool is_in_character(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & IN_CHARACTER) != 0;
}
static inline bool has_fwdlist(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & HAS_FWDLIST) != 0;
}
static inline bool has_listen(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & HAS_LISTEN) != 0;
}

static inline bool is_robot_player(GameDatabase *database, DbRef x) {
  return is_player(database, x) && is_robot(database, x);
}
static inline bool is_good_owner(GameDatabase *database, DbRef x) {
  return is_good_obj(database, x) && is_owns_others(database, x);
}
static inline bool is_inherits(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & INHERIT) != 0 ||
         (game_object_flags(database, game_object_owner(database, x)) &
          INHERIT) != 0 ||
         x == game_object_owner(database, x);
}
static inline bool is_wizard(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & WIZARD) ||
         ((game_object_flags(database, game_object_owner(database, x)) &
           WIZARD) &&
          is_inherits(database, x));
}
static inline bool is_enter_ok(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & ENTER_OK) != 0 &&
         has_location(database, x) && has_contents(database, x);
}
static inline bool is_suspect(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, game_object_owner(database, x)) &
          SUSPECT) != 0;
}
static inline bool is_hidden(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & DARK) ||
         (game_object_flags2(database, x) & UNFINDABLE);
}
static inline bool is_connected(GameDatabase *database, DbRef x) {
  return (game_object_flags2(database, x) & CONNECTED) != 0 &&
         typeof_obj(database, x) == TYPE_PLAYER;
}

static inline bool is_alive(GameDatabase *database, DbRef x) {
  return is_player(database, x) ||
         (is_puppet(database, x) && has_contents(database, x));
}
static inline bool is_dark(GameDatabase *database, DbRef x) {
  return (game_object_flags(database, x) & DARK) != 0 &&
         (is_wizard(database, x) || !is_alive(database, x));
}
// Defined in flags.c, where the configuration definition is visible.
bool is_safe(GameDatabase *database, const ServerConfiguration *configuration,
             DbRef x, DbRef p);

static inline bool is_on_enter_lock(EvaluationContext *evaluation, DbRef p,
                                    DbRef x) {
  return check_zone(evaluation, p, x);
}

bool is_examinable(EvaluationContext *evaluation, DbRef p, DbRef x);
bool is_myopic_exam(EvaluationContext *evaluation, DbRef p, DbRef x);
bool is_controls(EvaluationContext *evaluation, DbRef p, DbRef x);

static inline bool is_parentable(EvaluationContext *evaluation, DbRef p,
                                 DbRef x) {
  return is_controls(evaluation, p, x);
}

// Defined in flags.c, where database and configuration types are visible.
void mark(GameDatabase *database, DbRef x);
void unmark(GameDatabase *database, DbRef x);
bool is_marked(GameDatabase *database, DbRef x);
void unmark_all(GameDatabase *database);

bool can_link_exit(EvaluationContext *evaluation, DbRef p, DbRef x);
bool is_linkable(EvaluationContext *evaluation, DbRef p, DbRef x);

// Defined in flags.c, where attrs.h's AF_* flags are visible.
bool see_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
              DbRef o, long f);
bool see_attr_explicit(GameDatabase *database, DbRef p, DbRef x, Attribute *a,
                       DbRef o, long f);
bool set_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
              long f);
bool read_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
               DbRef o, long f);
bool write_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
                long f);

static inline void s_opaque(GameDatabase *database, DbRef x) {
  game_object_set_flags(database, x, game_object_flags(database, x) | OPAQUE);
}
static inline void s_fixed(GameDatabase *database, DbRef x) {
  game_object_set_flags2(database, x, game_object_flags2(database, x) | FIXED);
}
static inline void s_halted(GameDatabase *database, DbRef x) {
  game_object_set_flags(database, x, game_object_flags(database, x) | HALT);
}
static inline void s_going(GameDatabase *database, DbRef x) {
  game_object_set_flags(database, x, game_object_flags(database, x) | GOING);
}
static inline void s_connected(GameDatabase *database, DbRef x) {
  game_object_set_flags2(database, x,
                         game_object_flags2(database, x) | CONNECTED);
}
static inline void s_hardcode(GameDatabase *database, DbRef x) {
  game_object_set_flags2(database, x,
                         game_object_flags2(database, x) | HARDCODE);
}
static inline void c_hardcode(GameDatabase *database, DbRef x) {
  game_object_set_flags2(database, x,
                         game_object_flags2(database, x) & ~HARDCODE);
}
static inline void s_zombie(GameDatabase *database, DbRef x) {
  game_object_set_flags2(database, x, game_object_flags2(database, x) | ZOMBIE);
}
static inline void c_connected(GameDatabase *database, DbRef x) {
  game_object_set_flags2(database, x,
                         game_object_flags2(database, x) & ~CONNECTED);
}
static inline void s_in_character(GameDatabase *database, DbRef x) {
  game_object_set_flags2(database, x,
                         game_object_flags2(database, x) | IN_CHARACTER);
}

static inline char *unparse_flags(GameDatabase *database, DbRef p, DbRef t) {
  return decode_flags(database, p, game_object_flags(database, t),
                      game_object_flags2(database, t),
                      game_object_flags3(database, t));
}

static inline void s_dark(GameDatabase *database, DbRef x) {
  game_object_set_flags(database, x, game_object_flags(database, x) | DARK);
}
