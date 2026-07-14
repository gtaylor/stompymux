
/* powers.h - object powers */

/* $Id: powers.h,v 1.3 2005/06/23 02:59:58 murrayma Exp $ */

#pragma once

#include "db.h"
#include "rbtab.h"

#define POWER_EXT 0x1 /* Lives in extended powers word */

/* First word of powers */
/* 0x00000001 is reserved for the removed quota power. */
/* 0x00000002 through 0x00000080 are reserved for removed powers. */
#define POW_FIND_UNFIND 0x00000100 /* Can find unfindable players */
/* 0x00000200 is reserved for the removed free_money power. */
/* 0x00000400 is reserved for the removed free_quota power. */
/* 0x00000800 is reserved for the removed hide power. */
#define POW_IDLE 0x00001000 /* No idle limit */
/* 0x00002000 is reserved for the removed search power. */
#define POW_LONGFINGERS 0x00004000 /* Can get/look/etc from a distance */
/* 0x00008000 is reserved for the removed prog power. */
#define POW_COMM_ALL 0x00080000 /* Channel wiz */
/* 0x00100000 is reserved for the removed see_queue power. */
#define POW_SEE_HIDDEN                                                         \
  0x00200000 /* Player can see hidden players on WHO list */
/* 0x00400000 is reserved for the removed monitor power. */
/* 0x00800000 is reserved for the removed poll power. */
#define POW_NO_DESTROY 0x01000000 /* Cannot be destroyed */
#define POW_PASS_LOCKS 0x04000000 /* Player can pass any lock */
/* 0x08000000 is reserved for the removed stat_any power. */
/* 0x10000000 is reserved for the removed steal_money power. */
/* 0x20000000 is reserved for the removed tel_anywhere power. */
/* 0x40000000 is reserved for the removed tel_anything power. */
/* 0x80000000 is reserved for the removed unkillable power. */

/* Second word of powers */
/* 0x00000001 is reserved for the removed builder power. */
/* Mech stuff: */
#define POW_MECH 0x00000002     /* access to mech cmd set */
#define POW_SECURITY 0x00000004 /* 'admin' - debug/comp */
#define POW_MECHREP 0x00000008  /* access to mechrep cmd set */
#define POW_MAP 0x00000010      /* map modifying powers */
#define POW_TEMPLATE 0x00000020 /* templating powers */
#define POW_TECH 0x00000040     /* can do the IC tech commands */

/* end of mech stuff */

/* ---------------------------------------------------------------------------
 * POWERENT: Information about object powers.
 */

typedef struct power_entry {
  const char *powername; /* Name of the flag */
  int powervalue;        /* Which bit in the object is the flag */
  int powerpower;        /* Ctrl flags for this power (recursive? :-) */
  int listperm;          /* Who sees this flag when set */
  int (*handler)(dbref, dbref, POWER, int,
                 int); /* Handler for setting/clearing this flag */
} POWERENT;

typedef struct powerset {
  POWER word1;
  POWER word2;
} POWERSET;

extern void init_powertab(void);
extern void display_powertab(dbref);
extern void power_set(dbref, dbref, char *, int);
extern char *power_description(dbref, dbref);
extern POWERENT *find_power(dbref, char *);
extern int has_power(dbref, dbref, char *);
extern void decompile_powers(dbref, dbref, char *);
extern int decode_power(dbref, char *, POWERSET *);

#define s_Change_Quotas(c) s_Powers((c), Powers(c) | POW_CHG_QUOTAS)
#define s_Find_Unfindable(c) s_Powers((c), Powers(c) | POW_FIND_UNFIND)
#define s_Can_Idle(c) s_Powers((c), Powers(c) | POW_IDLE)
#define s_Long_Fingers(c) s_Powers((c), Powers(c) | POW_LONGFINGERS)
#define s_Comm_All(c) s_Powers((c), Powers(c) | POW_COMM_ALL)
#define s_See_Hidden(c) s_Powers((c), Powers(c) | POW_SEE_HIDDEN)
#define s_No_Destroy(c) s_Powers((c), Powers(c) | POW_NO_DESTROY)
#define s_Set_Maint_Flags(c) s_Powers((c), Powers(c) | POW_SET_MFLAGS)

/* 'mech set-on macros */

#define s_Mech(c) s_Powers2((c), Powers2(c) | POW_MECH)
#define s_Security(c) s_Powers2((c), Powers2(c) | POW_SECURITY)
#define s_Mechrep(c) s_Powers2((c), Powers2(c) | POW_MECHREP)
#define s_Map(c) s_Powers2((c), Powers2(c) | POW_MAP)
#define s_Template(c) s_Powers2((c), Powers2(c) | POW_TEMPLATE)
#define s_Tech(c) s_Powers2((c), Powers2(c) | POW_TECH)

/* end of 'mech stuff */

#define Find_Unfindable(c) ((Powers(c) & POW_FIND_UNFIND) != 0)
#define Can_Idle(c) (((Powers(c) & POW_IDLE) != 0) || Wizard(c))
#define Long_Fingers(c) (((Powers(c) & POW_LONGFINGERS) != 0) || Wizard(c))
#define Comm_All(c) (((Powers(c) & POW_COMM_ALL) != 0) || Wizard(c))
#define See_Hidden(c) ((Powers(c) & POW_SEE_HIDDEN) != 0)
#define No_Destroy(c) (((Powers(c) & POW_NO_DESTROY) != 0) || Wizard(c))
#define Set_Maint_Flags(c) ((Powers(c) & POW_SET_MFLAGS) != 0)
#define Pass_Locks(c) ((Powers(c) & POW_PASS_LOCKS) != 0)

/* Mecha */
#define Mech(c) (((Powers2(c) & POW_MECH) != 0) || Wizard(c))
#define Security(c) (((Powers2(c) & POW_SECURITY) != 0) || Wizard(c))
#define Tech(c) (((Powers2(c) & POW_TECH) != 0) || Wizard(c))
#define Mechrep(c) (((Powers2(c) & POW_MECHREP) != 0) || Wizard(c))
#define Map(c) (((Powers2(c) & POW_MAP) != 0) || Wizard(c))
#define Template(c) (((Powers2(c) & POW_TEMPLATE) != 0) || Wizard(c))

/* End Mecha */
