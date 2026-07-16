
/* db.h - In-memory game-object, attribute, and lock data model. */

#pragma once

#include "mux/server/platform.h"

#include <sys/file.h>

#define ITER_PARENTS(t, p, l)                                                  \
  for ((l) = 0, (p) = (t);                                                     \
       (is_good_obj(p) && ((l) < mudconf.parent_nest_lim));                    \
       (p) = obj_parent(p), (l)++)

int get_atr(char *name);

typedef struct Attribute Attribute;
struct Attribute {
  const char *name; /* This has to be first.  braindeath. */
  int number;       /* attr number */
  int flags;
  int (*check)(int, DbRef, DbRef, int, char *);
};

typedef struct AttributeList AttributeList;
struct AttributeList {
  char *data; /* Attribute text. */
  int size;   /* Length of attribute */
  int number; /* Attribute number. */
};

typedef struct AttributeStack AttributeStack;
struct AttributeStack {
  char *data;
  AttributeStack *next;
};

typedef struct forward_list FWDLIST;

extern Attribute *attribute_by_number(int anum);
extern Attribute *attribute_by_name(const char *s);

extern Attribute attr_table[];

extern Attribute **anum_table;

static inline Attribute *anum_get(int x) { return anum_table[x]; }
static inline void anum_set(int x, Attribute *v) { anum_table[x] = v; }
extern void anum_extend(int);

constexpr char ATR_INFO_CHAR = '\1'; /* Leadin char for attr control data */

/* Boolean expressions, for locks */
constexpr int BOOLEXP_AND = 0;
constexpr int BOOLEXP_OR = 1;
constexpr int BOOLEXP_NOT = 2;
constexpr int BOOLEXP_CONST = 3;
constexpr int BOOLEXP_ATR = 4;
constexpr int BOOLEXP_INDIR = 5;
constexpr int BOOLEXP_CARRY = 6;
constexpr int BOOLEXP_IS = 7;
constexpr int BOOLEXP_OWNER = 8;
constexpr int BOOLEXP_EVAL = 9;

typedef struct BooleanExpression BooleanExpression;
struct BooleanExpression {
  boolexp_type type;
  struct BooleanExpression *sub1;
  struct BooleanExpression *sub2;
  DbRef thing; /* thing refers to an object */
};

constexpr BooleanExpression *TRUE_BOOLEXP = (BooleanExpression *)0;

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
  DbRef parent; /* ALL: defaults for attrs, exits, $cmds, */
  DbRef owner;  /* PLAYER: domain number + class + moreflags */
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
};

typedef char *NAME;

extern GameObject *db;
extern NAME *names;

static inline DbRef obj_location(DbRef t) { return db[t].location; }
static inline DbRef obj_zone(DbRef t) { return db[t].zone; }
static inline DbRef obj_contents(DbRef t) { return db[t].contents; }
static inline DbRef obj_exits(DbRef t) { return db[t].exits; }
static inline DbRef obj_next(DbRef t) { return db[t].next; }
static inline DbRef obj_link(DbRef t) { return db[t].link; }
static inline DbRef obj_owner(DbRef t) { return db[t].owner; }
static inline DbRef obj_parent(DbRef t) { return db[t].parent; }
static inline Flag obj_flags(DbRef t) { return db[t].flags; }
static inline Flag obj_flags2(DbRef t) { return db[t].flags2; }
static inline Flag obj_flags3(DbRef t) { return db[t].flags3; }
static inline Power obj_powers(DbRef t) { return db[t].powers; }
static inline Power obj_powers2(DbRef t) { return db[t].powers2; }
static inline AttributeStack *obj_stack(DbRef t) { return db[t].stackhead; }
static inline DbRef obj_home(DbRef t) { return obj_link(t); }
static inline DbRef obj_dropto(DbRef t) { return obj_location(t); }

static inline void s_location(DbRef t, DbRef n) { db[t].location = n; }
static inline void s_zone(DbRef t, DbRef n) { db[t].zone = n; }
static inline void s_contents(DbRef t, DbRef n) { db[t].contents = n; }
static inline void s_exits(DbRef t, DbRef n) { db[t].exits = n; }
static inline void s_next(DbRef t, DbRef n) { db[t].next = n; }
static inline void s_link(DbRef t, DbRef n) { db[t].link = n; }
static inline void s_owner(DbRef t, DbRef n) { db[t].owner = n; }
static inline void s_parent(DbRef t, DbRef n) { db[t].parent = n; }
static inline void s_flags(DbRef t, Flag n) { db[t].flags = n; }
static inline void s_flags2(DbRef t, Flag n) { db[t].flags2 = n; }
static inline void s_flags3(DbRef t, Flag n) { db[t].flags3 = n; }
static inline void s_powers(DbRef t, Power n) { db[t].powers = n; }
static inline void s_powers2(DbRef t, Power n) { db[t].powers2 = n; }
static inline void s_stack(DbRef t, AttributeStack *n) { db[t].stackhead = n; }
static inline void s_home(DbRef t, DbRef n) { s_link(t, n); }
static inline void s_dropto(DbRef t, DbRef n) { s_location(t, n); }

extern BooleanExpression *boolean_expression_duplicate(BooleanExpression *);
extern void boolean_expression_free(BooleanExpression *);
extern DbRef parse_dbref(const char *);
extern int mkattr(char *);
extern void al_add(DbRef, int);
extern void al_delete(DbRef, int);
extern void al_destroy(DbRef);
extern void al_store(void);
extern void init_attrtab(void);
extern void db_grow(DbRef);
extern void db_free(void);
extern void db_make_minimal(void);
extern void destroy_thing(DbRef);
extern void destroy_exit(DbRef);
extern int dump_database_internal(int);
int has_commands(DbRef thing);
void object_password_set(DbRef thing, const char *password);
void object_name_set(DbRef thing, char *name);
char *Name(DbRef thing);
char *PureName(DbRef thing);
int fwdlist_load(FWDLIST *list, DbRef player, char *string);
void fwdlist_set(DbRef player, FWDLIST *list);
void fwdlist_clr(DbRef player);
int fwdlist_rewrite(FWDLIST *list, char *string);
FWDLIST *fwdlist_get(DbRef player);
void init_min_db(void);
void attribute_stack_push(void);
void attribute_stack_pop(void);
int attribute_list_first(DbRef player, char **state);
int attribute_list_next(char **state);
int init_gdbm_db(char *path);
void attribute_copy(DbRef player, DbRef source, DbRef destination);
void attribute_change_owner(DbRef thing);
void attribute_clear(DbRef thing, int attribute_number);
void attribute_add_raw(DbRef thing, int attribute_number, char *value);
void attribute_add(DbRef thing, int attribute_number, char *value, DbRef owner,
                   long flags);
void attribute_set_owner(DbRef thing, int attribute_number, DbRef owner);
void attribute_set_flags(DbRef thing, int attribute_number, long flags);
char *attribute_get_raw(DbRef thing, int attribute_number);
char *attribute_get(DbRef thing, int attribute_number, DbRef *owner,
                    long *flags);
char *attribute_parent_get(DbRef thing, int attribute_number, DbRef *owner,
                           long *flags);
char *attribute_get_string(char *buffer, DbRef thing, int attribute_number,
                           DbRef *owner, long *flags);
char *attribute_parent_get_string(char *buffer, DbRef thing,
                                  int attribute_number, DbRef *owner,
                                  long *flags);
int attribute_get_info(DbRef thing, int attribute_number, DbRef *owner,
                       long *flags);
int attribute_parent_get_info(DbRef thing, int attribute_number, DbRef *owner,
                              long *flags);
void attribute_free(DbRef thing);
int check_zone(DbRef player, DbRef thing);
int check_zone_for_player(DbRef player, DbRef thing);
void toast_player(DbRef player);

#define DOLIST(thing, list)                                                    \
  for ((thing) = (list); ((thing) != NOTHING) && (obj_next(thing) != (thing)); \
       (thing) = obj_next(thing))
#define SAFE_DOLIST(thing, next, list)                                         \
  for ((thing) = (list),                                                       \
      (next) = ((thing) == NOTHING ? NOTHING : obj_next(thing));               \
       (thing) != NOTHING && (obj_next(thing) != (thing));                     \
       (thing) = (next), (next) = obj_next(next))
#define DO_WHOLE_DB(thing)                                                     \
  for ((thing) = 0; (thing) < mudstate.db_top; (thing)++)

#define DO_WHOLE_DB_REV(thing)                                                 \
  for ((thing) = mudstate.db_top - 1; (thing) > 0; (thing)--)

constexpr int DUMP_NORMAL = 0;
constexpr int DUMP_CRASHED = 1;
constexpr int DUMP_KILLED = 4;
