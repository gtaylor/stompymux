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
constexpr int HAS_STARTUP = 0x00400000; /* Load some attrs at startup */
constexpr int OPAQUE = 0x00800000;      /* Can't see inside */
constexpr int VERBOSE = 0x01000000;     /* Tells owner everything it does. */
constexpr int INHERIT = 0x02000000;     /* Gets owner's privs. (i.e. Wiz) */
constexpr int NOSPOOF = 0x04000000;     /* Report originator of all actions. */
constexpr int ROBOT = 0x08000000;       /* Player is a ROBOT */
constexpr int SAFE = 0x10000000;        /* Need /override to @destroy */
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
constexpr int AUDITORIUM = 0x00000100;  /* Should we check the SpeechLock? */
constexpr int ANSI = 0x00000200;
/* 0x00000400 is reserved for the removed REGISTERED flag. */
constexpr int FIXED = 0x00000800;
/* 0x00001000 is reserved for the removed UNINSPECTED flag. */
constexpr int NO_COMMAND = 0x00002000;

constexpr int NOBLEED = 0x00008000;
/* 0x00010000 is reserved for the removed STAFF flag. */
constexpr int HAS_DAILY = 0x00020000;
constexpr int GAGGED = 0x00040000;
constexpr int HARDCODE = 0x00080000;
constexpr int IN_CHARACTER = 0x00100000;
constexpr int ANSIMAP = 0x00200000; /* Player uses ANSI maps */
constexpr int HAS_HOURLY = 0x00400000;
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
  int (*handler)(DbRef, DbRef, Flag, int,
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

constexpr int OF_CONTENTS = 0x0001; /* Object has contents: obj_contents() */
constexpr int OF_LOCATION = 0x0002; /* Object has a location: obj_location() */
constexpr int OF_EXITS = 0x0004;    /* Object has exits: obj_exits() */
constexpr int OF_HOME = 0x0008;     /* Object has a home: obj_home() */
constexpr int OF_DROPTO = 0x0010;   /* Object has a dropto: obj_dropto() */
constexpr int OF_OWNER = 0x0020;    /* Object can own other objects */
constexpr int OF_SIBLINGS = 0x0040; /* Object has siblings: obj_next() */

typedef struct flagset {
  Flag word1;
  Flag word2;
  Flag word3;
} FLAGSET;

extern void init_flagtab(void);
extern void display_flagtab(DbRef);
extern void flag_set(DbRef, DbRef, char *, int);
extern char *flag_description(DbRef, DbRef);
extern FLAGENT *find_flag(DbRef, char *);
extern char *decode_flags(DbRef, Flag, long, long);
extern int has_flag(DbRef, DbRef, char *);
extern char *unparse_object(DbRef, DbRef, int);
extern char *unparse_object_numonly(DbRef);
extern int convert_flags(DbRef, char *, FLAGSET *, Flag *);

constexpr DbRef GOD = 1;

/* ---------------------- Object Permission/Attribute Functions */
/* is_flag_set(X,T,F) - Is X of type T and have flag F set? */
/* typeof(X)         - What object type is X */
/* is_god(X)            - Is X player #1 */
/* is_robot_player(X)          - Is X a robot player */
/* is_wizard(X)         - Does X have wizard privs */
/* is_alive(X)          - Is X a player or a puppet */
/* is_dark(X)           - Is X dark */
/* is_floating(X)       - Prevent 'disconnected room' msgs for room X */
/* is_quiet(X)       - Should 'Set.' messages et al from X be disabled */
/* is_verbose(X)        - Should owner receive all commands executed? */
/* is_trace(X)          - Should owner receive eval trace output? */
/* is_halted(X)         - Is X halted (not allowed to run commands)? */
/* is_suspect(X)        - Is X someone the wizzes should keep an eye on */
/* is_safe(X,P)         - Does P need the /OVERRIDE switch to @destroy X? */
/* is_monitor(X)        - Should we check for ^xxx:xxx listens on player? */
/* is_myopic(X)         - Should things as if we were nonowner/nonwiz */
/* is_audible(X)        - Should X forward messages? */
/* is_findable(X)       - Can @whereis find X */
/* is_hideout(X)        - Is @whereis blocked for X */
/* has_location(X)   - Is X something with a location (ie plyr or obj) */
/* has_home(X)       - Is X something with a home (ie plyr or obj) */
/* has_contents(X)   - Is X something with contents (ie plyr/obj/room) */
/* is_good_obj(X)       - Is X inside the DB and have a valid type? */
/* is_good_owner(X)  - Is X a good owner value? */
/* is_going(X)          - Is X marked GOING? */
/* is_inherits(X)       - Does X inherit the privs of its owner */
/* is_examinable(P,X)   - Can P look at attribs of X */
/* is_myopic_exam(P,X)  - Can P look at attribs of X (obeys MYOPIC) */
/* is_controls(P,X)     - Can P force X to do something */
/* can_link_exit(P,X) - Can P link from exit X */
/* is_linkable(P,X)     - Can P link to X */
/* mark(x)           - Set marked flag on X */
/* unmark(x)         - Clear marked flag on X */
/* is_marked(x)         - Check marked flag on X */
/* is_hardcode(x)       - Check hardcode flag on X */
/* is_in_character(x)   - Whether or not mecha's IC */
/* see_attr(P,X.A,O,F)  - Can P see text attr A on X if attr has owner O */
/* set_attr(P,X,A,F)    - Can P set/change text attr A (with flags F) on X */
/* read_attr(P,X,A,O,F) - Can P see attr A on X if attr has owner O */
/* write_attr(P,X,A,F)  - Can P set/change attr A (with flags F) on X */

static inline int typeof_obj(DbRef x) { return obj_flags(x) & TYPE_MASK; }
static inline bool is_flag_set(DbRef thing, int type, int flag) {
  return typeof_obj(thing) == type && (obj_flags(thing) & flag);
}
static inline bool is_god(DbRef x) { return x == GOD; }
static inline bool is_robot(DbRef x) { return (obj_flags(x) & ROBOT) != 0; }
static inline bool is_player(DbRef x) { return typeof_obj(x) == TYPE_PLAYER; }
static inline bool is_room(DbRef x) { return typeof_obj(x) == TYPE_ROOM; }
static inline bool is_exit(DbRef x) { return typeof_obj(x) == TYPE_EXIT; }
static inline bool is_thing(DbRef x) { return typeof_obj(x) == TYPE_THING; }
static inline bool is_owns_others(DbRef x) {
  return (object_types[typeof_obj(x)].flags & OF_OWNER) != 0;
}
static inline bool has_location(DbRef x) {
  return (object_types[typeof_obj(x)].flags & OF_LOCATION) != 0;
}
static inline bool has_contents(DbRef x) {
  return (object_types[typeof_obj(x)].flags & OF_CONTENTS) != 0;
}
static inline bool has_exits(DbRef x) {
  return (object_types[typeof_obj(x)].flags & OF_EXITS) != 0;
}
static inline bool has_siblings(DbRef x) {
  return (object_types[typeof_obj(x)].flags & OF_SIBLINGS) != 0;
}
static inline bool has_home(DbRef x) {
  return (object_types[typeof_obj(x)].flags & OF_HOME) != 0;
}
static inline bool has_dropto(DbRef x) {
  return (object_types[typeof_obj(x)].flags & OF_DROPTO) != 0;
}
// Defined in flags.c, where server_state.h's full mudstate type is visible.
bool is_good_obj(DbRef x);
static inline bool is_fixed(DbRef x) { return (obj_flags2(x) & FIXED) != 0; }
static inline bool is_ansi(DbRef x) { return (obj_flags2(x) & ANSI) != 0; }
static inline bool is_ansimap(DbRef x) {
  return (obj_flags2(x) & ANSIMAP) != 0;
}
static inline bool is_no_command(DbRef x) {
  return (obj_flags2(x) & NO_COMMAND) != 0;
}
static inline bool is_transparent(DbRef x) {
  return (obj_flags(x) & SEETHRU) != 0;
}
static inline bool is_sticky(DbRef x) { return (obj_flags(x) & STICKY) != 0; }
static inline bool is_quiet(DbRef x) { return (obj_flags(x) & QUIET) != 0; }
static inline bool is_halted(DbRef x) { return (obj_flags(x) & HALT) != 0; }
static inline bool is_trace(DbRef x) { return (obj_flags(x) & TRACE) != 0; }
static inline bool is_going(DbRef x) { return (obj_flags(x) & GOING) != 0; }
static inline bool is_monitor(DbRef x) { return (obj_flags(x) & MONITOR) != 0; }
static inline bool is_myopic(DbRef x) { return (obj_flags(x) & MYOPIC) != 0; }
static inline bool is_puppet(DbRef x) { return (obj_flags(x) & PUPPET) != 0; }
static inline bool is_opaque(DbRef x) { return (obj_flags(x) & OPAQUE) != 0; }
static inline bool is_verbose(DbRef x) { return (obj_flags(x) & VERBOSE) != 0; }
static inline bool is_nospoof(DbRef x) { return (obj_flags(x) & NOSPOOF) != 0; }
static inline bool is_audible(DbRef x) {
  return (obj_flags(x) & HEARTHRU) != 0;
}
static inline bool is_gagged(DbRef x) { return (obj_flags2(x) & GAGGED) != 0; }
static inline bool has_key_flag(DbRef x) { return (obj_flags2(x) & KEY) != 0; }
static inline bool is_auditorium(DbRef x) {
  return (obj_flags2(x) & AUDITORIUM) != 0;
}
static inline bool is_floating(DbRef x) {
  return (obj_flags2(x) & FLOATING) != 0;
}
static inline bool is_findable(DbRef x) {
  return (obj_flags2(x) & UNFINDABLE) == 0;
}
static inline bool is_hideout(DbRef x) {
  return (obj_flags2(x) & UNFINDABLE) != 0;
}
static inline bool is_light(DbRef x) { return (obj_flags2(x) & LIGHT) != 0; }
static inline bool is_hardcode(DbRef x) {
  return (obj_flags2(x) & HARDCODE) != 0;
}
static inline bool is_zombie(DbRef x) { return (obj_flags2(x) & ZOMBIE) != 0; }
static inline bool is_in_character(DbRef x) {
  return (obj_flags2(x) & IN_CHARACTER) != 0;
}
static inline bool has_fwdlist(DbRef x) {
  return (obj_flags2(x) & HAS_FWDLIST) != 0;
}
static inline bool has_listen(DbRef x) {
  return (obj_flags2(x) & HAS_LISTEN) != 0;
}

static inline bool is_robot_player(DbRef x) {
  return is_player(x) && is_robot(x);
}
static inline bool is_good_owner(DbRef x) {
  return is_good_obj(x) && is_owns_others(x);
}
static inline bool is_inherits(DbRef x) {
  return (obj_flags(x) & INHERIT) != 0 ||
         (obj_flags(obj_owner(x)) & INHERIT) != 0 || x == obj_owner(x);
}
static inline bool is_wizard(DbRef x) {
  return (obj_flags(x) & WIZARD) ||
         ((obj_flags(obj_owner(x)) & WIZARD) && is_inherits(x));
}
static inline bool is_enter_ok(DbRef x) {
  return (obj_flags(x) & ENTER_OK) != 0 && has_location(x) && has_contents(x);
}
static inline bool is_suspect(DbRef x) {
  return (obj_flags2(obj_owner(x)) & SUSPECT) != 0;
}
static inline bool is_hidden(DbRef x) {
  return (obj_flags(x) & DARK) || (obj_flags2(x) & UNFINDABLE);
}
static inline bool is_connected(DbRef x) {
  return (obj_flags2(x) & CONNECTED) != 0 && typeof_obj(x) == TYPE_PLAYER;
}

static inline bool is_alive(DbRef x) {
  return is_player(x) || (is_puppet(x) && has_contents(x));
}
static inline bool is_dark(DbRef x) {
  return (obj_flags(x) & DARK) != 0 && (is_wizard(x) || !is_alive(x));
}
// Defined in flags.c, where server_state.h's full mudconf type is visible.
bool is_safe(DbRef x, DbRef p);

static inline bool is_on_enter_lock(DbRef p, DbRef x) {
  return check_zone(p, x);
}

static inline bool is_examinable(DbRef p, DbRef x) {
  return is_wizard(p) || (obj_owner(p) == obj_owner(x)) ||
         is_on_enter_lock(p, x);
}

static inline bool is_myopic_exam(DbRef p, DbRef x) {
  return !is_myopic(p) && (is_wizard(p) || (obj_owner(p) == obj_owner(x)) ||
                           is_on_enter_lock(p, x));
}

static inline bool is_controls(DbRef p, DbRef x) {
  return is_good_obj(x) && !(is_god(x) && !is_god(p)) &&
         (is_wizard(p) ||
          ((obj_owner(p) == obj_owner(x)) &&
           (is_inherits(p) || !is_inherits(x))) ||
          is_on_enter_lock(p, x));
}

static inline bool is_parentable(DbRef p, DbRef x) { return is_controls(p, x); }

// Defined in flags.c, where server_state.h's full mudstate/mudconf types are
// visible.
void mark(DbRef x);
void unmark(DbRef x);
bool is_marked(DbRef x);
void unmark_all(void);

static inline bool can_link_exit(DbRef p, DbRef x) {
  return typeof_obj(x) == TYPE_EXIT &&
         (obj_location(x) == NOTHING || is_controls(p, x));
}
static inline bool is_linkable(DbRef p, DbRef x) {
  return is_good_obj(x) && has_contents(x) && is_controls(p, x);
}

// Defined in flags.c, where attrs.h's AF_* flags are visible.
bool see_attr(DbRef p, DbRef x, Attribute *a, DbRef o, long f);
bool see_attr_explicit(DbRef p, DbRef x, Attribute *a, DbRef o, long f);
bool set_attr(DbRef p, DbRef x, Attribute *a, long f);
bool read_attr(DbRef p, DbRef x, Attribute *a, DbRef o, long f);
bool write_attr(DbRef p, DbRef x, Attribute *a, long f);

static inline void s_opaque(DbRef x) { s_flags(x, obj_flags(x) | OPAQUE); }
static inline void s_fixed(DbRef x) { s_flags2(x, obj_flags2(x) | FIXED); }
static inline void s_halted(DbRef x) { s_flags(x, obj_flags(x) | HALT); }
static inline void s_going(DbRef x) { s_flags(x, obj_flags(x) | GOING); }
static inline void s_connected(DbRef x) {
  s_flags2(x, obj_flags2(x) | CONNECTED);
}
static inline void s_hardcode(DbRef x) {
  s_flags2(x, obj_flags2(x) | HARDCODE);
}
static inline void c_hardcode(DbRef x) {
  s_flags2(x, obj_flags2(x) & ~HARDCODE);
}
static inline void s_zombie(DbRef x) { s_flags2(x, obj_flags2(x) | ZOMBIE); }
static inline void c_connected(DbRef x) {
  s_flags2(x, obj_flags2(x) & ~CONNECTED);
}
static inline void s_in_character(DbRef x) {
  s_flags2(x, obj_flags2(x) | IN_CHARACTER);
}

static inline char *unparse_flags(DbRef p, DbRef t) {
  return decode_flags(p, obj_flags(t), obj_flags2(t), obj_flags3(t));
}

static inline void s_dark(DbRef x) { s_flags(x, obj_flags(x) | DARK); }
