/* command.h - Command parsing, dispatch, and command-handler declarations. */

#include "mux/server/platform.h"

#pragma once

#include "mux/database/db.h"
#include "mux/support/name_table.h"

int check_access(DbRef player, int mask);
void set_prefix_cmds(void);

/* from comsys.c */

void do_cemit(DbRef, DbRef, int, char *, char *);       /* channel emit */
void do_chboot(DbRef, DbRef, int, char *, char *);      /* channel boot */
void do_editchannel(DbRef, DbRef, int, char *, char *); /* edit a channel */
void do_checkchannel(DbRef, DbRef, int, char *);        /* check a channel */
void do_createchannel(DbRef, DbRef, int, char *);       /* create a channel */
void do_destroychannel(DbRef, DbRef, int, char *);      /* destroy a channel */
void do_edituser(DbRef, DbRef, int, char *, char *); /* edit a channel user */
void do_chanlist(DbRef, DbRef, int);               /* gives a channel listing */
void do_chanstatus(DbRef, DbRef, int, char *);     /* gives channelstatus */
void do_chopen(DbRef, DbRef, int, char *, char *); /* opens a channel */
void do_channelwho(DbRef, DbRef, int, char *);     /* who's on a channel */
void do_addcom(DbRef, DbRef, int, char *, char *); /* adds a comalias */
void do_allcom(DbRef, DbRef, int, char *); /* on, off, who, all aliases */
void do_comlist(DbRef, DbRef, int);        /* channel who by alias */
void do_comtitle(DbRef, DbRef, int, char *,
                 char *);                  /* sets a title on a channel */
void do_clearcom(DbRef, DbRef, int);       /* clears all comaliases */
void do_delcom(DbRef, DbRef, int, char *); /* deletes a comalias */
void do_tapcom(DbRef, DbRef, int, char *, char *); /* taps a channel */

void do_admin(DbRef, DbRef, int, char *, char *); /* Change config parameters */
void do_alias(DbRef, DbRef, int, char *,
              char *); /* Change the alias of something */
void do_attribute(DbRef, DbRef, int, char *,
                  char *);               /* Manage user-named attributes */
void do_boot(DbRef, DbRef, int, char *); /* Force-disconnect a player */
void do_chown(DbRef, DbRef, int, char *,
              char *); /* Change object or attribute owner */
void do_chownall(DbRef, DbRef, int, char *,
                 char *); /* Give away all of someone's objs */
void do_chzone(DbRef, DbRef, int, char *,
               char *); /* Change an object's zone. */
void do_clone(DbRef, DbRef, int, char *,
              char *);              /* Create a copy of an object */
void do_comment(DbRef, DbRef, int); /* Ignore argument and do nothing */
void do_cpattr(DbRef, DbRef, int, char *, char *[], int); /* Copy attributes */
void do_create(DbRef, DbRef, int, char *, char *); /* Create a new object */
void do_cut(DbRef, DbRef, int, char *); /* Truncate contents or exits list */
void do_dbck(DbRef, DbRef, int);        /* Consistency check */
void do_destroy(DbRef, DbRef, int, char *);            /* Destroy an object */
void do_dig(DbRef, DbRef, int, char *, char *[], int); /* Dig a new room */
void do_dolist(DbRef, DbRef, int, char *, char *, char *[],
               int);                     /* Iterate command on list members */
void do_drop(DbRef, DbRef, int, char *); /* Drop an object */
void do_dump(DbRef, DbRef, int);         /* Dump the database */
void do_edit(DbRef, DbRef, int, char *, char *[],
             int);                            /* Edit one or more attributes */
void do_enter(DbRef, DbRef, int, char *);     /* Enter an object */
void do_entrances(DbRef, DbRef, int, char *); /* List exits and links to loc */
void do_examine(DbRef, DbRef, int, char *);   /* Examine an object */
void do_find(DbRef, DbRef, int, char *);      /* Search for name in database */
void do_fixdb(DbRef, DbRef, int, char *,
              char *); /* Database repair functions */
void do_force(DbRef, DbRef, int, char *, char *, char *[],
              int); /* Force someone to do something */
void do_force_prefixed(DbRef, DbRef, int, char *, char *[],
                       int); /* #<num> <cmd> variant of FORCE */
void do_function(DbRef, DbRef, int, char *,
                 char *);               /* Make iser-def global function */
void do_get(DbRef, DbRef, int, char *); /* Get an object */
void do_give(DbRef, DbRef, int, char *, char *); /* Give something away */
void do_global(DbRef, DbRef, int, char *);  /* Enable/disable global flags */
void do_halt(DbRef, DbRef, int, char *);    /* Remove commands from the queue */
void do_help(DbRef, DbRef, int, char *);    /* Print info from help files */
void do_helpreload(DbRef, DbRef, int);      /* Reindex help articles */
void do_history(DbRef, DbRef, int, char *); /* View various history info */
void do_multis(DbRef, DbRef, int);
void do_inventory(DbRef, DbRef, int);            /* Print what I am carrying */
void do_last(DbRef, DbRef, int, char *);         /* Get recent login info */
void do_leave(DbRef, DbRef, int);                /* Leave the current object */
void do_link(DbRef, DbRef, int, char *, char *); /* Set home, dropto, or dest */
void do_luaparent(DbRef, DbRef, int, char *, char *);
void do_luareload(DbRef, DbRef, int);
void do_list(DbRef, DbRef, int, char *); /* List contents of internal tables */
void do_list_file(DbRef, DbRef, int,
                  char *); /* List contents of message files */
void do_lock(DbRef, DbRef, int, char *, char *); /* Set a lock on an object */
void do_pagelock(DbRef, DbRef, int, char *, char *); /* Sets a Pagelock */
void do_pageunlock(DbRef, DbRef, int, char *);       /* Removes a Pagelock */
void do_look(DbRef, DbRef, int, char *); /* Look here or at something */
void do_move(DbRef, DbRef, int, char *); /* Move about using exits */
void do_mvattr(DbRef, DbRef, int, char *, char *[],
               int); /* Move attributes on object */
void do_mudwho(DbRef, DbRef, int, char *,
               char *); /* WHO for inter-mud page/who suppt */
void do_name(DbRef, DbRef, int, char *,
             char *); /* Change the name of something */
void do_newpassword(DbRef, DbRef, int, char *, char *); /* Change passwords */
void do_notify(DbRef, DbRef, int, char *,
               char *); /* Notify or drain semaphore */
void do_open(DbRef, DbRef, int, char *, char *[], int); /* Open an exit */
void do_page(DbRef, DbRef, int, char *,
             char *); /* Send message to faraway player */
void do_parent(DbRef, DbRef, int, char *, char *);   /* Set parent field */
void do_password(DbRef, DbRef, int, char *, char *); /* Change my password */
void do_pcreate(DbRef, DbRef, int, char *, char *);  /* Create new characters */
void do_pemit(DbRef, DbRef, int, char *,
              char *); /* Messages to specific player */
void do_power(DbRef, DbRef, int, char *, char *); /* Sets powers */
void do_ps(DbRef, DbRef, int, char *);            /* List contents of queue */
void do_queue(DbRef, DbRef, int, char *);         /* Force queue processing */
void do_quit(DbRef, DbRef, int);                  /* Disconnect this session */
void do_readcache(DbRef, DbRef, int);             /* Reread text file cache */
void do_say(DbRef, DbRef, int, char *);           /* Messages to all */
void do_search(DbRef, DbRef, int,
               char *); /* Search for objs matching criteria */
void do_set(DbRef, DbRef, int, char *, char *); /* Set flags or attributes */
void do_setattr(DbRef, DbRef, int, char *, char *); /* Set object attribute */
void do_setvattr(DbRef, DbRef, int, char *,
                 char *);                    /* Set variable attribute */
void do_shutdown(DbRef, DbRef, int, char *); /* Stop the game */
void do_stats(DbRef, DbRef, int, char *);    /* Display object type breakdown */
void do_sweep(DbRef, DbRef, int, char *);    /* Check for listeners */
void do_switch(DbRef, DbRef, int, char *, char *[], int, char *[],
               int); /* Execute cmd based on match */
void do_teleport(DbRef, DbRef, int, char *, char *); /* Teleport elsewhere */
void do_think(DbRef, DbRef, int, char *);            /* Think command */
void do_timewarp(DbRef, DbRef, int, char *);         /* Warp various timers */
void do_trigger(DbRef, DbRef, int, char *, char *[],
                int);                      /* Trigger an attribute */
void do_unlock(DbRef, DbRef, int, char *); /* Remove a lock from an object */
void do_unlink(DbRef, DbRef, int, char *); /* Unlink exit or remove dropto */
void do_use(DbRef, DbRef, int, char *);    /* Use object */
void do_version(DbRef, DbRef, int);        /* List MUX version number */
void do_verb(DbRef, DbRef, int, char *, char *[],
             int); /* Execute a user-created verb */
void do_wait(DbRef, DbRef, int, char *, char *, char *[],
             int);                       /* Perform command after a wait */
void do_wipe(DbRef, DbRef, int, char *); /* Mass-remove attrs from obj */
void do_who(DbRef, DbRef, int, char *);  /* Wizard WHO listing */
void do_dbclean(DbRef, DbRef, int);      /* Remove stale vattr entries */
void do_addcommand(DbRef, DbRef, int, char *,
                   char *); /* Add or replace a global command */
void do_delcommand(DbRef, DbRef, int, char *,
                   char *); /* Delete an added global command */
void do_listcommands(DbRef, DbRef, int,
                     char *); /* List added global commands */
/* from log.c */
#ifdef ARBITRARY_LOGFILES
void do_log(DbRef, DbRef, int, char *,
            char *); /* Log to arbitrary logfile in 'logs' */
#endif

/* Mecha stuff */
void do_show(DbRef, DbRef, int, char *, char *);
void do_charclear(DbRef, DbRef, int, char *);
void do_show_stat(DbRef, DbRef, int);

/*
 * A command's handler is either a real function pointer (dispatched via one
 * of the ((sig *)handler.fn)(...) casts in process_cmdent, chosen by the
 * CS_* flags in callseq) or, for softcode-added commands (CS_ADDED), a
 * pointer to the ADDENT chain of attributes implementing it. Storing both
 * kinds through a single `void *` isn't portable ISO C; a union keeps each
 * access through its real type instead.
 */
typedef union cmdentry_handler {
  GenericFnPtr fn;
  struct addedentry *added;
} CmdHandler;

typedef struct cmdentry CMDENT;
struct cmdentry {
  const char *cmdname;
  NameTable *switches;
  int perms;
  int extra;
  int callseq;
  CmdHandler handler;
};

void init_cmdtab(void);
int cf_access(int *vp, char *str, long extra, DbRef player, char *cmd);
int cf_acmd_access(int *vp, char *str, long extra, DbRef player, char *cmd);
int cf_attr_access(int *vp, char *str, long extra, DbRef player, char *cmd);
int cf_cmd_alias(void *vp, char *str, long extra, DbRef player, char *cmd);

typedef struct addedentry ADDENT;
struct addedentry {
  DbRef thing;
  int atr;
  char *name;
  struct addedentry *next;
};

/* Command handler call conventions */

constexpr int CS_NO_ARGS = 0x0000;   /* No arguments */
constexpr int CS_ONE_ARG = 0x0001;   /* One argument */
constexpr int CS_TWO_ARG = 0x0002;   /* Two arguments */
constexpr int CS_NARG_MASK = 0x0003; /* Argument count mask */
constexpr int CS_ARGV = 0x0004;      /* ARG2 is in ARGV form */
constexpr int CS_INTERP = 0x0010;    /* Interpret ARG2 if 2 args, ARG1 if 1 */
constexpr int CS_NOINTERP = 0x0020;  /* Never interp ARG2 if 2 or ARG1 if 1 */
constexpr int CS_CAUSE = 0x0040;     /* Pass cause to old command handler */
constexpr int CS_UNPARSE = 0x0080; /* Pass unparsed cmd to old-style handler */
constexpr int CS_CMDARG = 0x0100;  /* Pass in given command args */
constexpr int CS_STRIP = 0x0200;   /* Strip braces even when not interpreting */
constexpr int CS_STRIP_AROUND =
    0x0400;                         /* Strip braces around entire string only */
constexpr int CS_ADDED = 0X0800;    /* Command has been added by @addcommand */
constexpr int CS_NO_MACRO = 0x1000; /* Command can't be used inside macro */
constexpr int CS_LEADIN = 0x2000;   /* Command is a single-letter lead-in */

/* Command permission flags */

constexpr int CA_PUBLIC = 0x00000000; /* No access restrictions */
constexpr int CA_GOD = 0x00000001;    /* GOD only... */
constexpr int CA_WIZARD = 0x00000002; /* Wizards only */
/* 0x00000004 is reserved for the removed builder power restriction. */
/* 0x00000008 is reserved for the removed immortal restriction. */
constexpr int CA_ROBOT = 0x00000010; /* Robots only */
/* 0x00000020 is reserved for the removed announce power restriction. */
constexpr int CA_ADMIN = 0x00000800; /* Wizard */
/* 0x00001000 is reserved for the removed no_haven restriction. */
constexpr int CA_NO_ROBOT = 0x00002000; /* Not by ROBOT players */
/* 0x00004000 is reserved for the removed no_slave restriction. */
constexpr int CA_NO_SUSPECT = 0x00008000; /* Not by SUSPECT players */
constexpr int CA_NO_IC = 0x00020000;      /* Not by IC players */

constexpr int CA_GBL_BUILD = 0x01000000; /* Requires the global BUILDING flag */
constexpr int CA_GBL_INTERP = 0x02000000; /* Requires the global INTERP flag */
constexpr int CA_DISABLED = 0x04000000;   /* Command completely disabled */
constexpr int CA_LOCATION = 0x10000000;   /* Invoker must have location */
constexpr int CA_CONTENTS = 0x20000000;   /* Invoker must have contents */
constexpr int CA_PLAYER = 0x40000000;     /* Invoker must be a player */
// Stored as int (not unsigned) so it ORs cleanly into CMDENT.perms without
// signedness conversions; C23 guarantees the top-bit pattern converts to
// INT_MIN deterministically.
constexpr int CF_DARK = (int)0x80000000U; /* Command doesn't show up in list */

int check_access(DbRef, int);
void process_command(DbRef, DbRef, int, char *, char *[], int);
