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

#define ToUpper(C) (((C) >= 'a' && (C) <= 'z') ? (C) - 'a' + 'A' : (C))
#define ToLower(C) (((C) >= 'A' && (C) <= 'Z') ? (C) - 'A' + 'a' : (C))
#define safe_atoi(s) ((s == NULL) ? 0 : atoi(s))

/* Notifications are defined by game.h; these compatibility macros remain here.
 */
#define notify(p, m)                                                           \
  notify_checked(p, p, m, MSG_PUP_ALWAYS | MSG_ME_ALL | MSG_F_DOWN)
#define notify_quiet(p, m) notify_checked(p, p, m, MSG_PUP_ALWAYS | MSG_ME)
#define notify_with_cause(p, c, m)                                             \
  notify_checked(p, c, m, MSG_PUP_ALWAYS | MSG_ME_ALL | MSG_F_DOWN)
#define notify_quiet_with_cause(p, c, m)                                       \
  notify_checked(p, c, m, MSG_PUP_ALWAYS | MSG_ME)
#define notify_puppet(p, c, m) notify_checked(p, c, m, MSG_ME_ALL | MSG_F_DOWN)
#define notify_quiet_puppet(p, c, m) notify_checked(p, c, m, MSG_ME)
#define notify_all(p, c, m)                                                    \
  notify_checked(p, c, m,                                                      \
                 MSG_ME_ALL | MSG_NBR_EXITS | MSG_F_UP | MSG_F_CONTENTS)
#define notify_all_from_inside(p, c, m)                                        \
  notify_checked(p, c, m,                                                      \
                 MSG_ME_ALL | MSG_NBR_EXITS_A | MSG_F_UP | MSG_F_CONTENTS |    \
                     MSG_S_INSIDE)
#define notify_all_from_outside(p, c, m)                                       \
  notify_checked(p, c, m,                                                      \
                 MSG_ME_ALL | MSG_NBR_EXITS | MSG_F_UP | MSG_F_CONTENTS |      \
                     MSG_S_OUTSIDE)
/* Command handler keys */
#define ATTRIB_ACCESS 1    /* Change access to attribute */
#define ATTRIB_RENAME 2    /* Rename attribute */
#define ATTRIB_DELETE 4    /* Delete attribute */
#define BOOT_QUIET 1       /* Inhibit boot message to victim */
#define BOOT_PORT 2        /* Boot by port number */
#define CEMIT_NOHEADER 1   /* Channel emit without header */
#define CHOWN_ONE 1        /* item = new_owner */
#define CHOWN_ALL 2        /* old_owner = new_owner */
#define CLIST_FULL 1       /* Full listing of channels */
#define CSTATUS_FULL 1     /* Full listing of channel */
#define CLONE_LOCATION 0   /* Create cloned object in my location */
#define CLONE_INHERIT 1    /* Keep INHERIT bit if set */
#define CLONE_PRESERVE 2   /* Preserve the owner of the object */
#define CLONE_INVENTORY 4  /* Create cloned object in my inventory */
#define CLONE_SET_LOC 16   /* ARG2 is location of cloned object */
#define CLONE_SET_NAME 32  /* ARG2 is alternate name of cloned object */
#define CLONE_PARENT 64    /* Set parent on obj instd of cloning attrs */
#define CRE_INVENTORY 0    /* Create object in my inventory */
#define CRE_LOCATION 1     /* Create object in my location */
#define CRE_SET_LOC 2      /* ARG2 is location of new object */
#define CSET_PUBLIC 0      /* Sets a channel public */
#define CSET_PRIVATE 1     /* Sets a channel private (default) */
#define CSET_LOUD 2        /* Channel shows connects and disconnects */
#define CSET_QUIET 3       /* Channel doesn't show connects/disconnects */
#define CSET_LIST 4        /* Lists channels */
#define CSET_OBJECT 5      /* Sets the channel object for the channel */
#define CSET_STATUS 6      /* Gives status of channel */
#define CSET_TRANSPARENT 7 /* Set channel 'transparent' */
#define CSET_OPAQUE 8      /* Set channel !transparent */
#define DBCK_DEFAULT 1     /* Get default tests too */
#define DBCK_REPORT 2      /* Report info to invoker */
#define DBCK_FULL 4        /* Do all tests */
#define DBCK_FLOATING 8    /* Look for floating rooms */
#define DBCK_PURGE 16      /* Purge the db of refs to going objects */
#define DBCK_LINKS 32      /* Validate exit and object chains */
#define DBCK_WEALTH 64     /* Validate object value/wealth */
#define DBCK_OWNER 128     /* Do more extensive owner checking */
#define DBCK_OWN_EXIT 256  /* Check exit owner owns src or dest */
#define DBCK_WIZARD 512    /* Check for wizards/wiz objects */
#define DBCK_TYPES 1024    /* Check for valid & appropriate types */
#define DBCK_SPARE 2048    /* Make sure spare header fields are NOTHING */
#define DBCK_HOMES 4096    /* Make sure homes and droptos are valid */
#define DEST_ONE 1         /* object */
#define DEST_ALL 2         /* owner */
#define DEST_OVERRIDE 4    /* override Safe() */
#define DEST_RECURSIVE 8
#define DIG_TELEPORT 1     /* teleport to room after @digging */
#define DOLIST_SPACE 0     /* expect spaces as delimiter */
#define DOLIST_DELIMIT 1   /* expect custom delimiter */
#define DROP_QUIET 1       /* Don't do odrop/adrop if control */
#define DUMP_STRUCT 1      /* Dump flat structure file */
#define DUMP_TEXT 2        /* Dump text (gdbm) file */
#define DUMP_OPTIMIZE 3    /* Reorganize the gdbm file */
#define EXAM_DEFAULT 0     /* Default */
#define EXAM_BRIEF 1       /* Nonowner sees just owner */
#define EXAM_LONG 2        /* Nonowner sees public attrs too */
#define EXAM_DEBUG 4       /* Display more info for finding db problems */
#define EXAM_PARENT 8      /* Get attr from parent when exam obj/attr */
#define FIXDB_OWNER 1      /* Fix OWNER field */
#define FIXDB_LOC 2        /* Fix LOCATION field */
#define FIXDB_CON 4        /* Fix CONTENTS field */
#define FIXDB_EXITS 8      /* Fix EXITS field */
#define FIXDB_NEXT 16      /* Fix NEXT field */
#define FIXDB_ZONE 64      /* Fix ZONE field */
#define FIXDB_LINK 128     /* Fix LINK field */
#define FIXDB_PARENT 256   /* Fix PARENT field */
#define FIXDB_DEL_PN 512   /* Remove player name from player name index */
#define FIXDB_ADD_PN 1024  /* Add player name to player name index */
#define FIXDB_NAME 2048    /* Set NAME attribute */
#define FRC_PREFIX 0       /* #num command */
#define FRC_COMMAND 1      /* what=command */
#define GET_QUIET 1        /* Don't do osucc/asucc if control */
#define GIVE_QUIET 64      /* Inhibit give messages */
#define GLOB_ENABLE 1      /* key to enable */
#define GLOB_DISABLE 2     /* key to disable */
#define GLOB_LIST 3        /* key to list */
#define HALT_ALL 1         /* halt everything */
#define HELP_HELP 1        /* get data from help file */
#define HELP_WIZHELP 2     /* get data from wizard help file */
#define LOOK_LOOK 1        /* list desc (and succ/fail if room) */
#define LOOK_EXAM 2        /* full listing of object */
#define LOOK_DEXAM 3       /* debug listing of object */
#define LOOK_INVENTORY 4   /* list inventory of object */
#define LOOK_SCORE 5       /* list score (# coins) */
#define LOOK_OUTSIDE 8     /* look for object in container of player */
#define MOVE_QUIET 1       /* Dont do osucc/ofail/asucc/afail if ctrl */
#define NFY_NFY 0          /* Notify first waiting command */
#define NFY_NFYALL 1       /* Notify all waiting commands */
#define NFY_DRAIN 2        /* Delete waiting commands */
#define OPEN_LOCATION 0    /* Open exit in my location */
#define OPEN_INVENTORY 1   /* Open exit in me */
#define PASS_ANY 1         /* name=newpass */
#define PASS_MINE 2        /* oldpass=newpass */
#define PCRE_PLAYER 1      /* create new player */
#define PCRE_ROBOT 2       /* create robot player */
#define PEMIT_PEMIT 1      /* emit to named player */
#define PEMIT_OEMIT 2      /* emit to all in current room except named */
#define PEMIT_FSAY 3       /* force controlled obj to say */
#define PEMIT_FEMIT 4      /* force controlled obj to emit */
#define PEMIT_FPOSE 5      /* force controlled obj to pose */
#define PEMIT_FPOSE_NS 6   /* force controlled obj to pose w/o space */
#define PEMIT_CONTENTS 8   /* Send to contents (additive) */
#define PEMIT_HERE 16      /* Send to location (@femit, additive) */
#define PEMIT_ROOM 32      /* Send to containing rm (@femit, additive) */
#define PEMIT_LIST 64      /* Send to a list */
#define PS_BRIEF 0         /* Short PS report */
#define PS_LONG 1          /* Long PS report */
#define PS_SUMM 2          /* Queue counts only */
#define PS_ALL 4           /* List entire queue */
#define QUEUE_KICK 1       /* Process commands from queue */
#define QUEUE_WARP 2       /* Advance or set back wait queue clock */
#define SAY_SAY 1          /* say in current room */
#define SAY_NOSPACE 1      /* OR with xx_EMIT to get nospace form */
#define SAY_POSE 2         /* pose in current room */
#define SAY_POSE_NOSPC 3   /* pose w/o space in current room */
#define SAY_PREFIX 4       /* first char indicates foratting */
#define SAY_EMIT 5         /* emit in current room */
#define SAY_SHOUT 8        /* shout to all logged-in players */
#define SAY_WALLPOSE 9     /* Pose to all logged-in players */
#define SAY_WALLEMIT 10    /* Emit to all logged-in players */
#define SAY_WIZSHOUT 12    /* shout to all logged-in wizards */
#define SAY_WIZPOSE 13     /* Pose to all logged-in wizards */
#define SAY_WIZEMIT 14     /* Emit to all logged-in wizards */
#define SAY_ADMINSHOUT 15  /* Emit to all wizards */
#define SAY_GRIPE 16       /* Complain to management */
#define SAY_NOTE 17        /* Comment to log for wizards */
#define SAY_NOTAG 32       /* Don't put Broadcast: in front (additive) */
#define SAY_HERE 64        /* Output to current location */
#define SAY_ROOM 128       /* Output to containing room */
#define SET_QUIET 1        /* Don't display 'Set.' message. */
#define SHUTDN_NORMAL 0    /* Normal shutdown */
#define SHUTDN_PANIC 1     /* Write a panic dump file */
#define SHUTDN_EXIT 2      /* Exit from shutdown code */
#define SHUTDN_COREDUMP 4  /* Produce a coredump */
#define SHUTDN_KILLED 8    /* Preserve an operator-requested killed snapshot */
#define SRCH_SEARCH 1      /* Do a normal search */
#define SRCH_MARK 2        /* Set mark bit for matches */
#define SRCH_UNMARK 3      /* Clear mark bit for matches */
#define STAT_PLAYER 0      /* Display stats for one player or tot objs */
#define STAT_ALL 1         /* Display global stats */
#define STAT_ME 2          /* Display stats for me */
#define SWITCH_DEFAULT 0   /* Use the configured default for switch */
#define SWITCH_ANY 1       /* Execute all cases that match */
#define SWITCH_ONE 2       /* Execute only first case that matches */
#define SWEEP_ME 1         /* Check my inventory */
#define SWEEP_HERE 2       /* Check my location */
#define SWEEP_COMMANDS 4   /* Check for $-commands */
#define SWEEP_LISTEN 8     /* Check for @listen-ers */
#define SWEEP_PLAYER 16    /* Check for players and puppets */
#define SWEEP_CONNECT 32   /* Search for connected players/puppets */
#define SWEEP_EXITS 64     /* Search the exits for audible flags */
#define SWEEP_SCAN 128     /* Scan for pattern matching */
#define SWEEP_VERBOSE 256  /* Display what pattern matches */
#define TELEPORT_DEFAULT 1 /* Emit all messages */
#define TELEPORT_QUIET 2   /* Teleport in quietly */
#define TRIG_QUIET 1       /* Don't display 'Triggered.' message. */
#define TWARP_QUEUE 1      /* Warp the wait and sem queues */
#define TWARP_DUMP 2       /* Warp the dump interval */
#define TWARP_CLEAN 4      /* Warp the cleaning interval */
#define TWARP_IDLE 8       /* Warp the idle check interval */

/* emprty		16 */
#define TWARP_EVENTS 32 /* Warp the events checking interval */

/* Hush codes for movement messages */

#define HUSH_ENTER 1 /* xENTER/xEFAIL */
#define HUSH_LEAVE 2 /* xLEAVE/xLFAIL */
#define HUSH_EXIT 4  /* xSUCC/xDROP/xFAIL from exits */

/* Evaluation directives */

#define EV_FMASK 0x00000300        /* Mask for function type check */
#define EV_FIGNORE 0x00000000      /* Don't look for func if () found */
#define EV_FMAND 0x00000100        /* Text before () must be func name */
#define EV_FCHECK 0x00000200       /* Check text before () for function */
#define EV_STRIP 0x00000400        /* Strip one level of brackets */
#define EV_EVAL 0x00000800         /* Evaluate results before returning */
#define EV_STRIP_TS 0x00001000     /* Strip trailing spaces */
#define EV_STRIP_LS 0x00002000     /* Strip leading spaces */
#define EV_STRIP_ESC 0x00004000    /* Strip one level of \ characters */
#define EV_STRIP_AROUND 0x00008000 /* Strip {} only at ends of string */
#define EV_TOP 0x00010000          /* This is a toplevel call to eval() */
#define EV_NOTRACE 0x00020000      /* Don't trace this call to eval */
#define EV_NO_COMPRESS 0x00040000  /* Don't compress spaces. */
#define EV_NO_LOCATION 0x00080000  /* Supresses %l */
#define EV_NOFCHECK 0x00100000     /* Do not evaluate functions! */

/* Termination directives */

#define PT_NOTHING 0x00000000
#define PT_BRACE 0x00000001
#define PT_BRACKET 0x00000002
#define PT_PAREN 0x00000004
#define PT_COMMA 0x00000008
#define PT_SEMI 0x00000010
#define PT_EQUALS 0x00000020
#define PT_SPACE 0x00000040

/* Message forwarding directives */

#define MSG_PUP_ALWAYS 1    /* Always forward msg to puppet own */
#define MSG_INV 2           /* Forward msg to contents */
#define MSG_INV_L 4         /* ... only if msg passes my @listen */
#define MSG_INV_EXITS 8     /* Forward through my audible exits */
#define MSG_NBR 16          /* Forward msg to neighbors */
#define MSG_NBR_A 32        /* ... only if I am audible */
#define MSG_NBR_EXITS 64    /* Also forward to neighbor exits */
#define MSG_NBR_EXITS_A 128 /* ... only if I am audible */
#define MSG_LOC 256         /* Send to my location */
#define MSG_LOC_A 512       /* ... only if I am audible */
#define MSG_FWDLIST 1024    /* Forward to my fwdlist members if aud */
#define MSG_ME 2048         /* Send to me */
#define MSG_S_INSIDE 4096   /* Originator is inside target */
#define MSG_S_OUTSIDE 8192  /* Originator is outside target */
#define MSG_COLORIZE 16384  /* Message needs to be given color */
/* #define FREE		32768	*/
#define MSG_ME_ALL (MSG_ME | MSG_INV_EXITS | MSG_FWDLIST)
#define MSG_F_CONTENTS (MSG_INV)
#define MSG_F_UP (MSG_NBR_A | MSG_LOC_A)
#define MSG_F_DOWN (MSG_INV_L)

/* Look primitive directives */

#define LK_IDESC 0x0001
/* 0x0002 is reserved for the removed TERSE look mode. */
#define LK_SHOWATTR 0x0004
#define LK_SHOWEXIT 0x0008

/* Exit visibility precalculation codes */

#define VE_LOC_XAM 0x01    /* Location is examinable */
#define VE_LOC_DARK 0x02   /* Location is dark */
#define VE_LOC_LIGHT 0x04  /* Location is light */
#define VE_BASE_XAM 0x08   /* Base location (pre-parent) is examinable */
#define VE_BASE_DARK 0x10  /* Base location (pre-parent) is dark */
#define VE_BASE_LIGHT 0x20 /* Base location (pre-parent) is light */

/* Signal handling directives */

#define STARTLOG(key, p, s)                                                    \
  if ((((key) & mudconf.log_options) != 0) && start_log(p, s))
#define ENDLOG end_log()
#define LOG_SIMPLE(key, p, s, m)                                               \
  STARTLOG(key, p, s) {                                                        \
    log_text(m);                                                               \
    ENDLOG;                                                                    \
  }

#define test_top() ((mudstate.qfirst != NULL) ? 1 : 0)
#define controls(p, x) Controls(p, x)
