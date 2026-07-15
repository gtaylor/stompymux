
/* db.h - In-memory game-object, attribute, and lock data model. */

#pragma once

#include "mux/server/platform.h"

#include <sys/file.h>

#define ITER_PARENTS(t, p, l)                                                  \
  for ((l) = 0, (p) = (t); (Good_obj(p) && ((l) < mudconf.parent_nest_lim));   \
       (p) = Parent(p), (l)++)

#define Hasprivs(x) Wizard(x)
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
extern Attribute *attribute_by_name(char *s);

extern Attribute attr_table[];

extern Attribute **anum_table;

#define anum_get(x) (anum_table[(x)])
#define anum_set(x, v) anum_table[(x)] = v
extern void anum_extend(int);

#define ATR_INFO_CHAR '\1' /* Leadin char for attr control data */

/* Boolean expressions, for locks */
#define BOOLEXP_AND 0
#define BOOLEXP_OR 1
#define BOOLEXP_NOT 2
#define BOOLEXP_CONST 3
#define BOOLEXP_ATR 4
#define BOOLEXP_INDIR 5
#define BOOLEXP_CARRY 6
#define BOOLEXP_IS 7
#define BOOLEXP_OWNER 8
#define BOOLEXP_EVAL 9

typedef struct BooleanExpression BooleanExpression;
struct BooleanExpression {
  boolexp_type type;
  struct BooleanExpression *sub1;
  struct BooleanExpression *sub2;
  DbRef thing; /* thing refers to an object */
};

#define TRUE_BOOLEXP ((BooleanExpression *)0)

/* special dbref's */
#define NOTHING (-1)   /* null dbref */
#define AMBIGUOUS (-2) /* multiple possibilities, for matchers */
#define HOME (-3)      /* virtual room, represents mover's home */
#define NOPERM (-4)    /* Error status, no permission */

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

#define Location(t) db[t].location

#define Zone(t) db[t].zone

#define Contents(t) db[t].contents
#define Exits(t) db[t].exits
#define Next(t) db[t].next
#define Link(t) db[t].link
#define Owner(t) db[t].owner
#define Parent(t) db[t].parent
#define Flags(t) db[t].flags
#define Flags2(t) db[t].flags2
#define Flags3(t) db[t].flags3
#define Powers(t) db[t].powers
#define Powers2(t) db[t].powers2
#define Stack(t) db[t].stackhead
#define Home(t) Link(t)
#define Dropto(t) Location(t)

#define i_Name(t)                                                              \
  if (mudconf.cache_names)                                                     \
    purenames[t] = NULL;

#define s_Location(t, n) db[t].location = (n)

#define s_Zone(t, n) db[t].zone = (n)

#define s_Contents(t, n) db[t].contents = (n)
#define s_Exits(t, n) db[t].exits = (n)
#define s_Next(t, n) db[t].next = (n)
#define s_Link(t, n) db[t].link = (n)
#define s_Owner(t, n) db[t].owner = (n)
#define s_Parent(t, n) db[t].parent = (n)
#define s_Flags(t, n) db[t].flags = (n)
#define s_Flags2(t, n) db[t].flags2 = (n)
#define s_Flags3(t, n) db[t].flags3 = (n)
#define s_Powers(t, n) db[t].powers = (n)
#define s_Powers2(t, n) db[t].powers2 = (n)
#define s_Stack(t, n) db[t].stackhead = (n)
#define s_Home(t, n) s_Link(t, n)
#define s_Dropto(t, n) s_Location(t, n)

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
  for ((thing) = (list); ((thing) != NOTHING) && (Next(thing) != (thing));     \
       (thing) = Next(thing))
#define SAFE_DOLIST(thing, next, list)                                         \
  for ((thing) = (list),                                                       \
      (next) = ((thing) == NOTHING ? NOTHING : Next(thing));                   \
       (thing) != NOTHING && (Next(thing) != (thing));                         \
       (thing) = (next), (next) = Next(next))
#define DO_WHOLE_DB(thing)                                                     \
  for ((thing) = 0; (thing) < mudstate.db_top; (thing)++)

#define DO_WHOLE_DB_REV(thing)                                                 \
  for ((thing) = mudstate.db_top - 1; (thing) > 0; (thing)--)

#define Dropper(thing) (Connected(Owner(thing)) && is_hearer(thing))

#define DUMP_NORMAL 0
#define DUMP_CRASHED 1
#define DUMP_KILLED 4
