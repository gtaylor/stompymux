/* command.h - Command parsing, dispatch, and command-handler declarations. */

#include "mux/server/platform.h"

#pragma once

#include "mux/commands/command_invocation.h"

#include "mux/database/db.h"
#include "mux/support/name_table.h"

typedef struct CommandContext CommandContext;
typedef struct CommandRegistry CommandRegistry;
typedef struct ConfigurationContext ConfigurationContext;
typedef struct ServerConfiguration ServerConfiguration;

int check_access(GameDatabase *database,
                 const ServerConfiguration *configuration, DbRef player,
                 int mask);
void set_prefix_cmds(CommandRegistry *registry);

/* from comsys.c */

void do_chan(CommandInvocation *invocation);         /* administer channels */
void do_checkchannel(DbRef, DbRef, int, char *);     /* check a channel */
void do_edituser(DbRef, DbRef, int, char *, char *); /* edit a channel user */
void do_addcom(CommandInvocation *invocation);       /* adds a comalias */
void do_allcom(CommandInvocation *invocation);       /* operates on aliases */
void do_comlist(CommandInvocation *invocation);      /* channel who by alias */
void do_clearcom(CommandInvocation *invocation);     /* clears aliases */
void do_delcom(CommandInvocation *invocation);       /* deletes a comalias */
void do_tapcom(DbRef, DbRef, int, char *, char *);   /* taps a channel */

void do_admin(CommandInvocation *invocation); /* Change config parameters */
void do_alias(CommandInvocation *invocation);
void do_attribute(CommandInvocation *invocation); /* Manage user attributes. */
void do_boot(CommandInvocation *invocation);
void do_chown(CommandInvocation *invocation);
void do_chownall(CommandInvocation *invocation);
void do_chzone(CommandInvocation *invocation);
void do_clone(CommandInvocation *invocation);
void do_comment(DbRef, DbRef, int); /* Ignore argument and do nothing */
void do_cpattr(CommandInvocation *invocation); /* Copy attributes */
void do_create(CommandInvocation *invocation);
void do_cut(CommandInvocation *invocation);
void do_dbck(CommandInvocation *invocation); /* Consistency check */
void do_destroy(CommandInvocation *invocation);
void do_dig(CommandInvocation *invocation);
void do_dolist(CommandInvocation *invocation); /* Iterate over a list. */
void do_drop(CommandInvocation *invocation);   /* Drop an object */
void do_dump(CommandInvocation *invocation);   /* Dump the database */
void do_edit(CommandInvocation *invocation);
void do_enter(CommandInvocation *invocation);     /* Enter an object */
void do_entrances(CommandInvocation *invocation); /* List links to location. */
void do_examine(CommandInvocation *invocation);   /* Examine an object. */
void do_find(CommandInvocation *invocation);      /* Search the database. */
void do_fixdb(CommandInvocation *invocation); /* Database repair functions */
void do_force(CommandInvocation *invocation);
void do_force_prefixed(CommandInvocation *invocation); /* #num cmd FORCE */
void do_function(CommandInvocation *invocation); /* Define global function */
void do_get(CommandInvocation *invocation);      /* Get an object */
void do_give(CommandInvocation *invocation);     /* Give something away. */
void do_global(CommandInvocation *invocation);
void list_global_controls(EvaluationContext *evaluation,
                          ServerConfiguration *configuration, DbRef player);
void do_halt(CommandInvocation *invocation); /* Remove commands from queue */
void do_help(CommandInvocation *invocation); /* Print info from help files */
void do_helpreload(CommandInvocation *invocation); /* Reindex help articles */
void do_history(DbRef, DbRef, int, char *); /* View various history info */
void do_multis(DbRef, DbRef, int);
void do_inventory(CommandInvocation *invocation); /* Print carried objects. */
void do_last(CommandInvocation *invocation);      /* Get recent login info */
void do_leave(CommandInvocation *invocation);     /* Leave the current object */
void do_link(CommandInvocation *invocation);
void do_luaparent(CommandInvocation *invocation);
void do_luacheck(CommandInvocation *invocation);
void do_luareload(CommandInvocation *invocation);
void do_luaschedule(CommandInvocation *invocation);
void do_list(CommandInvocation *invocation); /* List internal tables. */
void do_list_file(CommandInvocation *invocation);
void do_lock(CommandInvocation *invocation); /* Set a lock on an object */
void do_look(CommandInvocation *invocation); /* Look here or at something. */
void do_move(CommandInvocation *invocation); /* Move about using exits */
void do_mvattr(CommandInvocation *invocation);
void do_mudwho(DbRef, DbRef, int, char *,
               char *); /* WHO for inter-mud page/who suppt */
void do_name(CommandInvocation *invocation);
void do_newpassword(CommandInvocation *invocation);
void do_notify(CommandInvocation *invocation); /* Notify or drain semaphore */
void do_open(CommandInvocation *invocation);
void do_page(CommandInvocation *invocation); /* Message a faraway player. */
void do_parent(CommandInvocation *invocation);
void do_password(CommandInvocation *invocation); /* Change my password */
void do_pcreate(CommandInvocation *invocation);
void do_pemit(CommandInvocation *invocation); /* Message a specific object. */
void do_power(CommandInvocation *invocation); /* Sets powers */
void do_ps(CommandInvocation *invocation);    /* List contents of queue */
void do_queue(CommandInvocation *invocation); /* Force queue processing */
void do_quit(CommandInvocation *invocation);  /* Disconnect this session */
void do_readcache(CommandInvocation *invocation); /* Reread text file cache */
void do_say(CommandInvocation *invocation);       /* Messages to all. */
void do_search(CommandInvocation *invocation);    /* Search matching objects. */
void do_set(CommandInvocation *invocation);
void do_setattr(CommandInvocation *invocation); /* Set object attribute */
void do_setvattr(CommandInvocation *invocation);
void do_shutdown(CommandInvocation *invocation); /* Stop the game */
void do_stats(CommandInvocation *invocation);  /* Display object statistics. */
void do_sweep(CommandInvocation *invocation);  /* Check for listeners. */
void do_switch(CommandInvocation *invocation); /* Execute cmd based on match */
void do_teleport(CommandInvocation *invocation);
void do_think(CommandInvocation *invocation);    /* Think command. */
void do_timewarp(CommandInvocation *invocation); /* Warp various timers */
void do_trigger(CommandInvocation *invocation);  /* Trigger an attribute */
void do_unlock(CommandInvocation *invocation);   /* Remove an object lock */
void do_unlink(CommandInvocation *invocation);
void do_use(CommandInvocation *invocation);     /* Use object. */
void do_version(CommandInvocation *invocation); /* List MUX version number */
void do_verb(CommandInvocation *invocation); /* Execute a user-created verb */
void do_wait(CommandInvocation *invocation); /* Perform command after wait */
void do_wipe(CommandInvocation *invocation);
void do_session(CommandInvocation *invocation); /* Wizard session listing */
void do_who(CommandInvocation *invocation);     /* Wizard WHO listing */
void do_dbclean(CommandInvocation *invocation); /* Remove stale vattr entries */
void do_addcommand(CommandInvocation *invocation);
void do_delcommand(CommandInvocation *invocation);
void do_listcommands(CommandInvocation *invocation);
/* from log.c */
#ifdef ARBITRARY_LOGFILES
void do_log(CommandInvocation *invocation); /* Log to arbitrary logfile */
#endif

/* Mecha stuff */
void do_show(CommandInvocation *invocation);
void do_charclear(CommandInvocation *invocation);
void do_show_stat(CommandInvocation *invocation);

/*
 * A command is either dispatched through the uniform typed invocation
 * boundary or, for softcode-added commands, through an ADDENT chain.
 */
typedef union cmdentry_handler {
  CommandInvocationHandler invoke;
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

void init_cmdtab(CommandRegistry *registry);
int cf_access(int *vp, char *str, long extra, DbRef player, char *cmd,
              ConfigurationContext *context);
int cf_acmd_access(int *vp, char *str, long extra, DbRef player, char *cmd,
                   ConfigurationContext *context);
int cf_attr_access(int *vp, char *str, long extra, DbRef player, char *cmd,
                   ConfigurationContext *context);
int cf_cmd_alias(void *vp, char *str, long extra, DbRef player, char *cmd,
                 ConfigurationContext *context);

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

/* 0x01000000 is reserved for the removed global building restriction. */
constexpr int CA_GBL_INTERP = 0x02000000; /* Requires the global INTERP flag */
constexpr int CA_DISABLED = 0x04000000;   /* Command completely disabled */
constexpr int CA_LOCATION = 0x10000000;   /* Invoker must have location */
constexpr int CA_CONTENTS = 0x20000000;   /* Invoker must have contents */
constexpr int CA_PLAYER = 0x40000000;     /* Invoker must be a player */
// Stored as int (not unsigned) so it ORs cleanly into CMDENT.perms without
// signedness conversions; C23 guarantees the top-bit pattern converts to
// INT_MIN deterministically.
constexpr int CF_DARK = (int)0x80000000U; /* Command doesn't show up in list */

void process_command(CommandContext *context, char *command, char *arguments[],
                     int argument_count);
