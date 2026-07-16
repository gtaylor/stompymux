/* server_api.h - Temporary aggregate of cross-domain MUX server interfaces. */

#include "mux/server/platform.h"

#pragma once

#include "mux/commands/command_queue.h"
#include "mux/commands/eval.h"
#include "mux/commands/help.h"
#include "mux/commands/look.h"
#include "mux/commands/verbs.h"
#include "mux/database/db.h"
#include "mux/network/netcommon.h"
#include "mux/network/telnet_socket.h"
#include "mux/server/configuration.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/log_cache.h"
#include "mux/server/server_state.h"
#include "mux/server/signals.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "mux/support/validation.h"
#include "mux/support/wild.h"
#include "mux/world/access.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/object.h"
#include "mux/world/object_list.h"
#include "mux/world/object_set.h"
#include "mux/world/object_spatial.h"
#include "mux/world/player.h"
#include "mux/world/walkdb.h"

#ifndef _DB_C
#define INLINE
#else /* _DB_C */
#ifdef __GNUC__
#define INLINE inline
#else /* __GNUC__ */
#define INLINE
#endif /* __GNUC__ */
#endif /* _DB_C */

static inline char ToUpper(char C) {
  return (C >= 'a' && C <= 'z') ? C - 'a' + 'A' : C;
}
static inline char ToLower(char C) {
  return (C >= 'A' && C <= 'Z') ? C - 'A' + 'a' : C;
}
static inline int safe_atoi(const char *s) {
  return s == nullptr ? 0 : atoi(s);
}

/* Message forwarding directives */

constexpr int MSG_PUP_ALWAYS = 1;    /* Always forward msg to puppet own */
constexpr int MSG_INV = 2;           /* Forward msg to contents */
constexpr int MSG_INV_L = 4;         /* ... only if msg passes my @listen */
constexpr int MSG_INV_EXITS = 8;     /* Forward through my audible exits */
constexpr int MSG_NBR = 16;          /* Forward msg to neighbors */
constexpr int MSG_NBR_A = 32;        /* ... only if I am audible */
constexpr int MSG_NBR_EXITS = 64;    /* Also forward to neighbor exits */
constexpr int MSG_NBR_EXITS_A = 128; /* ... only if I am audible */
constexpr int MSG_LOC = 256;         /* Send to my location */
constexpr int MSG_LOC_A = 512;       /* ... only if I am audible */
constexpr int MSG_FWDLIST = 1024;    /* Forward to my fwdlist members if aud */
constexpr int MSG_ME = 2048;         /* Send to me */
constexpr int MSG_S_INSIDE = 4096;   /* Originator is inside target */
constexpr int MSG_S_OUTSIDE = 8192;  /* Originator is outside target */
constexpr int MSG_COLORIZE = 16384;  /* Message needs to be given color */
/* #define FREE		32768	*/
constexpr int MSG_ME_ALL = MSG_ME | MSG_INV_EXITS | MSG_FWDLIST;
constexpr int MSG_F_CONTENTS = MSG_INV;
constexpr int MSG_F_UP = MSG_NBR_A | MSG_LOC_A;
constexpr int MSG_F_DOWN = MSG_INV_L;

/* Notifications are defined by game.h; these compatibility functions remain
 * here. */
static inline void notify(DbRef p, const char *m) {
  notify_checked(p, p, m, MSG_PUP_ALWAYS | MSG_ME_ALL | MSG_F_DOWN);
}
static inline void notify_quiet(DbRef p, const char *m) {
  notify_checked(p, p, m, MSG_PUP_ALWAYS | MSG_ME);
}
static inline void notify_with_cause(DbRef p, DbRef c, const char *m) {
  notify_checked(p, c, m, MSG_PUP_ALWAYS | MSG_ME_ALL | MSG_F_DOWN);
}
static inline void notify_quiet_with_cause(DbRef p, DbRef c, const char *m) {
  notify_checked(p, c, m, MSG_PUP_ALWAYS | MSG_ME);
}
static inline void notify_puppet(DbRef p, DbRef c, const char *m) {
  notify_checked(p, c, m, MSG_ME_ALL | MSG_F_DOWN);
}
static inline void notify_quiet_puppet(DbRef p, DbRef c, const char *m) {
  notify_checked(p, c, m, MSG_ME);
}
static inline void notify_all(DbRef p, DbRef c, const char *m) {
  notify_checked(p, c, m,
                 MSG_ME_ALL | MSG_NBR_EXITS | MSG_F_UP | MSG_F_CONTENTS);
}
static inline void notify_all_from_inside(DbRef p, DbRef c, const char *m) {
  notify_checked(p, c, m,
                 MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP | MSG_F_CONTENTS |
                     MSG_S_INSIDE);
}
static inline void notify_all_from_outside(DbRef p, DbRef c, const char *m) {
  notify_checked(p, c, m,
                 MSG_ME_ALL | MSG_NBR_EXITS | MSG_F_UP | MSG_F_CONTENTS |
                     MSG_S_OUTSIDE);
}
/* Command handler keys */
constexpr int ATTRIB_ACCESS = 1;   /* Change access to attribute */
constexpr int ATTRIB_RENAME = 2;   /* Rename attribute */
constexpr int ATTRIB_DELETE = 4;   /* Delete attribute */
constexpr int BOOT_QUIET = 1;      /* Inhibit boot message to victim */
constexpr int BOOT_PORT = 2;       /* Boot by port number */
constexpr int CEMIT_NOHEADER = 1;  /* Channel emit without header */
constexpr int CHOWN_ONE = 1;       /* item = new_owner */
constexpr int CHOWN_ALL = 2;       /* old_owner = new_owner */
constexpr int CLIST_FULL = 1;      /* Full listing of channels */
constexpr int CSTATUS_FULL = 1;    /* Full listing of channel */
constexpr int CLONE_LOCATION = 0;  /* Create cloned object in my location */
constexpr int CLONE_INHERIT = 1;   /* Keep INHERIT bit if set */
constexpr int CLONE_PRESERVE = 2;  /* Preserve the owner of the object */
constexpr int CLONE_INVENTORY = 4; /* Create cloned object in my inventory */
constexpr int CLONE_SET_LOC = 16;  /* ARG2 is location of cloned object */
constexpr int CLONE_SET_NAME = 32; /* ARG2 is alternate name of cloned object */
constexpr int CLONE_PARENT = 64; /* Set parent on obj instd of cloning attrs */
constexpr int CRE_INVENTORY = 0; /* Create object in my inventory */
constexpr int CRE_LOCATION = 1;  /* Create object in my location */
constexpr int CRE_SET_LOC = 2;   /* ARG2 is location of new object */
constexpr int CSET_PUBLIC = 0;   /* Sets a channel public */
constexpr int CSET_PRIVATE = 1;  /* Sets a channel private (default) */
constexpr int CSET_LOUD = 2;     /* Channel shows connects and disconnects */
constexpr int CSET_QUIET = 3;    /* Channel doesn't show connects/disconnects */
constexpr int CSET_LIST = 4;     /* Lists channels */
constexpr int CSET_OBJECT = 5;   /* Sets the channel object for the channel */
constexpr int CSET_STATUS = 6;   /* Gives status of channel */
constexpr int CSET_TRANSPARENT = 7; /* Set channel 'transparent' */
constexpr int CSET_OPAQUE = 8;      /* Set channel !transparent */
constexpr int DBCK_DEFAULT = 1;     /* Get default tests too */
constexpr int DBCK_REPORT = 2;      /* Report info to invoker */
constexpr int DBCK_FULL = 4;        /* Do all tests */
constexpr int DBCK_FLOATING = 8;    /* Look for floating rooms */
constexpr int DBCK_PURGE = 16;      /* Purge the db of refs to going objects */
constexpr int DBCK_LINKS = 32;      /* Validate exit and object chains */
constexpr int DBCK_WEALTH = 64;     /* Validate object value/wealth */
constexpr int DBCK_OWNER = 128;     /* Do more extensive owner checking */
constexpr int DBCK_OWN_EXIT = 256;  /* Check exit owner owns src or dest */
constexpr int DBCK_WIZARD = 512;    /* Check for wizards/wiz objects */
constexpr int DBCK_TYPES = 1024;    /* Check for valid & appropriate types */
constexpr int DBCK_SPARE = 2048; /* Make sure spare header fields are NOTHING */
constexpr int DBCK_HOMES = 4096; /* Make sure homes and droptos are valid */
constexpr int DEST_ONE = 1;      /* object */
constexpr int DEST_ALL = 2;      /* owner */
constexpr int DEST_OVERRIDE = 4; /* override is_safe() */
constexpr int DEST_RECURSIVE = 8;
constexpr int DIG_TELEPORT = 1;   /* teleport to room after @digging */
constexpr int DOLIST_SPACE = 0;   /* expect spaces as delimiter */
constexpr int DOLIST_DELIMIT = 1; /* expect custom delimiter */
constexpr int DROP_QUIET = 1;     /* Don't do odrop/adrop if control */
constexpr int DUMP_STRUCT = 1;    /* Dump flat structure file */
constexpr int DUMP_TEXT = 2;      /* Dump text (gdbm) file */
constexpr int DUMP_OPTIMIZE = 3;  /* Reorganize the gdbm file */
constexpr int EXAM_DEFAULT = 0;   /* Default */
constexpr int EXAM_BRIEF = 1;     /* Nonowner sees just owner */
constexpr int EXAM_LONG = 2;      /* Nonowner sees public attrs too */
constexpr int EXAM_DEBUG = 4;   /* Display more info for finding db problems */
constexpr int EXAM_PARENT = 8;  /* Get attr from parent when exam obj/attr */
constexpr int FIXDB_OWNER = 1;  /* Fix OWNER field */
constexpr int FIXDB_LOC = 2;    /* Fix LOCATION field */
constexpr int FIXDB_CON = 4;    /* Fix CONTENTS field */
constexpr int FIXDB_EXITS = 8;  /* Fix EXITS field */
constexpr int FIXDB_NEXT = 16;  /* Fix NEXT field */
constexpr int FIXDB_ZONE = 64;  /* Fix ZONE field */
constexpr int FIXDB_LINK = 128; /* Fix LINK field */
constexpr int FIXDB_PARENT = 256; /* Fix PARENT field */
constexpr int FIXDB_DEL_PN =
    512; /* Remove player name from player name index */
constexpr int FIXDB_ADD_PN = 1024; /* Add player name to player name index */
constexpr int FIXDB_NAME = 2048;   /* Set NAME attribute */
constexpr int FRC_PREFIX = 0;      /* #num command */
constexpr int FRC_COMMAND = 1;     /* what=command */
constexpr int GET_QUIET = 1;       /* Don't do osucc/asucc if control */
constexpr int GIVE_QUIET = 64;     /* Inhibit give messages */
constexpr int GLOB_ENABLE = 1;     /* key to enable */
constexpr int GLOB_DISABLE = 2;    /* key to disable */
constexpr int GLOB_LIST = 3;       /* key to list */
constexpr int HALT_ALL = 1;        /* halt everything */
constexpr int HELP_HELP = 1;       /* get data from help file */
constexpr int HELP_WIZHELP = 2;    /* get data from wizard help file */
constexpr int LOOK_LOOK = 1;       /* list desc (and succ/fail if room) */
constexpr int LOOK_EXAM = 2;       /* full listing of object */
constexpr int LOOK_DEXAM = 3;      /* debug listing of object */
constexpr int LOOK_INVENTORY = 4;  /* list inventory of object */
constexpr int LOOK_SCORE = 5;      /* list score (# coins) */
constexpr int LOOK_OUTSIDE = 8;    /* look for object in container of player */
constexpr int MOVE_QUIET = 1;      /* Dont do osucc/ofail/asucc/afail if ctrl */
constexpr int NFY_NFY = 0;         /* Notify first waiting command */
constexpr int NFY_NFYALL = 1;      /* Notify all waiting commands */
constexpr int NFY_DRAIN = 2;       /* Delete waiting commands */
constexpr int OPEN_LOCATION = 0;   /* Open exit in my location */
constexpr int OPEN_INVENTORY = 1;  /* Open exit in me */
constexpr int PASS_ANY = 1;        /* name=newpass */
constexpr int PASS_MINE = 2;       /* oldpass=newpass */
constexpr int PCRE_PLAYER = 1;     /* create new player */
constexpr int PCRE_ROBOT = 2;      /* create robot player */
constexpr int PEMIT_PEMIT = 1;     /* emit to named player */
constexpr int PEMIT_OEMIT = 2;    /* emit to all in current room except named */
constexpr int PEMIT_FSAY = 3;     /* force controlled obj to say */
constexpr int PEMIT_FEMIT = 4;    /* force controlled obj to emit */
constexpr int PEMIT_FPOSE = 5;    /* force controlled obj to pose */
constexpr int PEMIT_FPOSE_NS = 6; /* force controlled obj to pose w/o space */
constexpr int PEMIT_CONTENTS = 8; /* Send to contents (additive) */
constexpr int PEMIT_HERE = 16;    /* Send to location (@femit, additive) */
constexpr int PEMIT_ROOM = 32;    /* Send to containing rm (@femit, additive) */
constexpr int PEMIT_LIST = 64;    /* Send to a list */
constexpr int PS_BRIEF = 0;       /* Short PS report */
constexpr int PS_LONG = 1;        /* Long PS report */
constexpr int PS_SUMM = 2;        /* Queue counts only */
constexpr int PS_ALL = 4;         /* List entire queue */
constexpr int QUEUE_KICK = 1;     /* Process commands from queue */
constexpr int QUEUE_WARP = 2;     /* Advance or set back wait queue clock */
constexpr int SAY_SAY = 1;        /* say in current room */
constexpr int SAY_NOSPACE = 1;    /* OR with xx_EMIT to get nospace form */
constexpr int SAY_POSE = 2;       /* pose in current room */
constexpr int SAY_POSE_NOSPC = 3; /* pose w/o space in current room */
constexpr int SAY_PREFIX = 4;     /* first char indicates foratting */
constexpr int SAY_EMIT = 5;       /* emit in current room */
constexpr int SAY_SHOUT = 8;      /* shout to all logged-in players */
constexpr int SAY_WALLPOSE = 9;   /* Pose to all logged-in players */
constexpr int SAY_WALLEMIT = 10;  /* Emit to all logged-in players */
constexpr int SAY_WIZSHOUT = 12;  /* shout to all logged-in wizards */
constexpr int SAY_WIZPOSE = 13;   /* Pose to all logged-in wizards */
constexpr int SAY_WIZEMIT = 14;   /* Emit to all logged-in wizards */
constexpr int SAY_ADMINSHOUT = 15; /* Emit to all wizards */
constexpr int SAY_GRIPE = 16;      /* Complain to management */
constexpr int SAY_NOTE = 17;       /* Comment to log for wizards */
constexpr int SAY_NOTAG = 32;    /* Don't put Broadcast: in front (additive) */
constexpr int SAY_HERE = 64;     /* Output to current location */
constexpr int SAY_ROOM = 128;    /* Output to containing room */
constexpr int SET_QUIET = 1;     /* Don't display 'Set.' message. */
constexpr int SHUTDN_NORMAL = 0; /* Normal shutdown */
constexpr int SHUTDN_PANIC = 1;  /* Write a panic dump file */
constexpr int SHUTDN_EXIT = 2;   /* Exit from shutdown code */
constexpr int SHUTDN_COREDUMP = 4; /* Produce a coredump */
constexpr int SHUTDN_KILLED =
    8; /* Preserve an operator-requested killed snapshot */
constexpr int SRCH_SEARCH = 1;    /* Do a normal search */
constexpr int SRCH_MARK = 2;      /* Set mark bit for matches */
constexpr int SRCH_UNMARK = 3;    /* Clear mark bit for matches */
constexpr int STAT_PLAYER = 0;    /* Display stats for one player or tot objs */
constexpr int STAT_ALL = 1;       /* Display global stats */
constexpr int STAT_ME = 2;        /* Display stats for me */
constexpr int SWITCH_DEFAULT = 0; /* Use the configured default for switch */
constexpr int SWITCH_ANY = 1;     /* Execute all cases that match */
constexpr int SWITCH_ONE = 2;     /* Execute only first case that matches */
constexpr int SWEEP_ME = 1;       /* Check my inventory */
constexpr int SWEEP_HERE = 2;     /* Check my location */
constexpr int SWEEP_COMMANDS = 4; /* Check for $-commands */
constexpr int SWEEP_LISTEN = 8;   /* Check for @listen-ers */
constexpr int SWEEP_PLAYER = 16;  /* Check for players and puppets */
constexpr int SWEEP_CONNECT = 32; /* Search for connected players/puppets */
constexpr int SWEEP_EXITS = 64;   /* Search the exits for audible flags */
constexpr int SWEEP_SCAN = 128;   /* Scan for pattern matching */
constexpr int SWEEP_VERBOSE = 256;  /* Display what pattern matches */
constexpr int TELEPORT_DEFAULT = 1; /* Emit all messages */
constexpr int TELEPORT_QUIET = 2;   /* Teleport in quietly */
constexpr int TRIG_QUIET = 1;       /* Don't display 'Triggered.' message. */
constexpr int TWARP_QUEUE = 1;      /* Warp the wait and sem queues */
constexpr int TWARP_DUMP = 2;       /* Warp the dump interval */
constexpr int TWARP_CLEAN = 4;      /* Warp the cleaning interval */
constexpr int TWARP_IDLE = 8;       /* Warp the idle check interval */

/* emprty		16 */
constexpr int TWARP_EVENTS = 32; /* Warp the events checking interval */

/* Hush codes for movement messages */

constexpr int HUSH_ENTER = 1; /* xENTER/xEFAIL */
constexpr int HUSH_LEAVE = 2; /* xLEAVE/xLFAIL */
constexpr int HUSH_EXIT = 4;  /* xSUCC/xDROP/xFAIL from exits */

/* Evaluation directives */

constexpr int EV_FMASK = 0x00000300;     /* Mask for function type check */
constexpr int EV_FIGNORE = 0x00000000;   /* Don't look for func if () found */
constexpr int EV_FMAND = 0x00000100;     /* Text before () must be func name */
constexpr int EV_FCHECK = 0x00000200;    /* Check text before () for function */
constexpr int EV_STRIP = 0x00000400;     /* Strip one level of brackets */
constexpr int EV_EVAL = 0x00000800;      /* Evaluate results before returning */
constexpr int EV_STRIP_TS = 0x00001000;  /* Strip trailing spaces */
constexpr int EV_STRIP_LS = 0x00002000;  /* Strip leading spaces */
constexpr int EV_STRIP_ESC = 0x00004000; /* Strip one level of \ characters */
constexpr int EV_STRIP_AROUND =
    0x00008000;                        /* Strip {} only at ends of string */
constexpr int EV_TOP = 0x00010000;     /* This is a toplevel call to eval() */
constexpr int EV_NOTRACE = 0x00020000; /* Don't trace this call to eval */
constexpr int EV_NO_COMPRESS = 0x00040000; /* Don't compress spaces. */
constexpr int EV_NO_LOCATION = 0x00080000; /* Supresses %l */
constexpr int EV_NOFCHECK = 0x00100000;    /* Do not evaluate functions! */

/* Termination directives */

constexpr int PT_NOTHING = 0x00000000;
constexpr int PT_BRACE = 0x00000001;
constexpr int PT_BRACKET = 0x00000002;
constexpr int PT_PAREN = 0x00000004;
constexpr int PT_COMMA = 0x00000008;
constexpr int PT_SEMI = 0x00000010;
constexpr int PT_EQUALS = 0x00000020;
constexpr int PT_SPACE = 0x00000040;

/* Look primitive directives */

constexpr int LK_IDESC = 0x0001;
/* 0x0002 is reserved for the removed TERSE look mode. */
constexpr int LK_SHOWATTR = 0x0004;
constexpr int LK_SHOWEXIT = 0x0008;

/* Exit visibility precalculation codes */

constexpr int VE_LOC_XAM = 0x01;   /* Location is examinable */
constexpr int VE_LOC_DARK = 0x02;  /* Location is dark */
constexpr int VE_LOC_LIGHT = 0x04; /* Location is light */
constexpr int VE_BASE_XAM = 0x08; /* Base location (pre-parent) is examinable */
constexpr int VE_BASE_DARK = 0x10;  /* Base location (pre-parent) is dark */
constexpr int VE_BASE_LIGHT = 0x20; /* Base location (pre-parent) is light */

/* Signal handling directives */

#define STARTLOG(key, p, s)                                                    \
  if ((((key) & mudconf.log_options) != 0) && start_log(p, s))
#define ENDLOG end_log()
