/* powers.h - Object power definitions and power-management declarations. */

#pragma once

#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/support/hash_table.h"

typedef struct WorldContext WorldContext;

constexpr int POWER_EXT = 0x1; /* Lives in extended powers word */

/* First word of powers */
/* 0x00000001 is reserved for the removed quota power. */
/* 0x00000002 through 0x00000080 are reserved for removed powers. */
/* 0x00000100 is reserved. */
/* 0x00000200 is reserved for the removed free_money power. */
/* 0x00000400 is reserved for the removed free_quota power. */
/* 0x00000800 is reserved for the removed hide power. */
constexpr int POW_IDLE = 0x00001000; /* No idle limit */
/* 0x00002000 is reserved for the removed search power. */
constexpr int POW_LONGFINGERS =
    0x00004000; /* Can get/look/etc from a distance */
/* 0x00008000 is reserved for the removed prog power. */
constexpr int POW_COMM_ALL = 0x00080000; /* Channel wiz */
/* 0x00100000 is reserved for the removed see_queue power. */
constexpr int POW_SEE_HIDDEN =
    0x00200000; /* Player can see hidden players on WHO list */
/* 0x00400000 is reserved for the removed monitor power. */
/* 0x00800000 is reserved for the removed poll power. */
constexpr int POW_NO_DESTROY = 0x01000000; /* Cannot be destroyed */
constexpr int POW_PASS_LOCKS = 0x04000000; /* Player can pass any lock */
/* 0x08000000 is reserved for the removed stat_any power. */
/* 0x10000000 is reserved for the removed steal_money power. */
/* 0x20000000 is reserved for the removed tel_anywhere power. */
/* 0x40000000 is reserved for the removed tel_anything power. */
/* 0x80000000 is reserved for the removed unkillable power. */

/* Second word of powers */
/* 0x00000001 is reserved for the removed builder power. */
/* Mech stuff: */
constexpr int POW_MECH = 0x00000002;     /* access to mech cmd set */
constexpr int POW_SECURITY = 0x00000004; /* 'admin' - debug/comp */
constexpr int POW_MECHREP = 0x00000008;  /* access to mechrep cmd set */
constexpr int POW_MAP = 0x00000010;      /* map modifying powers */
constexpr int POW_TEMPLATE = 0x00000020; /* templating powers */
constexpr int POW_TECH = 0x00000040;     /* can do the IC tech commands */

/* end of mech stuff */

/* ---------------------------------------------------------------------------
 * POWERENT: Information about object powers.
 */

typedef struct power_entry {
  const char *powername; /* Name of the flag */
  int powervalue;        /* Which bit in the object is the flag */
  int powerpower;        /* Ctrl flags for this power (recursive? :-) */
  int listperm;          /* Who sees this flag when set */
  int (*handler)(EvaluationContext *, DbRef, DbRef, Power, int,
                 int); /* Handler for setting/clearing this flag */
} POWERENT;

typedef struct powerset {
  Power word1;
  Power word2;
} POWERSET;

typedef struct WorldIndexes WorldIndexes;
typedef struct EvaluationContext EvaluationContext;

extern void init_powertab(WorldIndexes *indexes);
extern void display_powertab(EvaluationContext *, DbRef);
extern void power_set(EvaluationContext *, WorldIndexes *, DbRef, DbRef, char *,
                      int);
extern char *power_description(GameDatabase *, DbRef, DbRef);
extern POWERENT *find_power(WorldIndexes *, DbRef, char *);
extern int has_power(WorldContext *, DbRef, DbRef, char *);
extern int decode_power(EvaluationContext *, WorldIndexes *, DbRef, char *,
                        POWERSET *);

static inline bool can_idle(GameDatabase *database, DbRef c) {
  return (game_object_powers(database, c) & POW_IDLE) != 0 ||
         is_wizard(database, c);
}
static inline bool is_long_fingers(GameDatabase *database, DbRef c) {
  return (game_object_powers(database, c) & POW_LONGFINGERS) != 0 ||
         is_wizard(database, c);
}
static inline bool is_comm_all(GameDatabase *database, DbRef c) {
  return (game_object_powers(database, c) & POW_COMM_ALL) != 0 ||
         is_wizard(database, c);
}
static inline bool is_pass_locks(GameDatabase *database, DbRef c) {
  return (game_object_powers(database, c) & POW_PASS_LOCKS) != 0;
}

/* Mecha */
static inline bool is_security(GameDatabase *database, DbRef c) {
  return (game_object_powers2(database, c) & POW_SECURITY) != 0 ||
         is_wizard(database, c);
}
static inline bool is_tech_power(GameDatabase *database, DbRef c) {
  return (game_object_powers2(database, c) & POW_TECH) != 0 ||
         is_wizard(database, c);
}
static inline bool is_template_power(GameDatabase *database, DbRef c) {
  return (game_object_powers2(database, c) & POW_TEMPLATE) != 0 ||
         is_wizard(database, c);
}

/* End Mecha */
