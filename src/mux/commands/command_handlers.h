/* Native command handler declarations used by the command table. */

#pragma once

#include "mux/commands/command_invocation.h"
#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

/* from comsys.c */

void do_checkchannel(DbRef, DbRef, int, char *);     /* check a channel */
void do_edituser(DbRef, DbRef, int, char *, char *); /* edit a channel user */
void do_tapcom(DbRef, DbRef, int, char *, char *);   /* taps a channel */

void do_admin(CommandInvocation *invocation); /* Change config parameters */
void do_attribute(CommandInvocation *invocation);
void do_alias(CommandInvocation *invocation);
void do_boot(CommandInvocation *invocation);
void do_chzone(CommandInvocation *invocation);
void do_color(CommandInvocation *invocation);
void do_clone(CommandInvocation *invocation);
void do_create(CommandInvocation *invocation);
void do_destroy(CommandInvocation *invocation);
void do_dig(CommandInvocation *invocation);
void do_dump(CommandInvocation *invocation); /* Dump the database */
void do_find(CommandInvocation *invocation); /* Search the database. */
void do_force(CommandInvocation *invocation);
void do_force_prefixed(CommandInvocation *invocation); /* #num cmd FORCE */
void do_give(CommandInvocation *invocation); /* Give something away. */
void do_global(CommandInvocation *invocation);
void list_global_controls(EvaluationContext *evaluation,
                          ServerConfiguration *configuration, DbRef player);
void do_halt(CommandInvocation *invocation); /* Remove commands from queue */
void do_history(DbRef, DbRef, int, char *);  /* View various history info */
void do_multis(DbRef, DbRef, int);
void do_link(CommandInvocation *invocation);
void do_lua(CommandInvocation *invocation);
void do_list(CommandInvocation *invocation); /* List internal tables. */
void do_look(CommandInvocation *invocation); /* Look here or at something. */
void do_mudwho(DbRef, DbRef, int, char *,
               char *); /* WHO for inter-mud page/who suppt */
void do_name(CommandInvocation *invocation);
void do_newpassword(CommandInvocation *invocation);
void do_open(CommandInvocation *invocation);
void do_page(CommandInvocation *invocation); /* Message a faraway player. */
void do_pcreate(CommandInvocation *invocation);
void do_pemit(CommandInvocation *invocation); /* Message a specific object. */
void do_power(CommandInvocation *invocation); /* Sets powers */
void do_quit(CommandInvocation *invocation);  /* Disconnect this session */
void do_readcache(CommandInvocation *invocation); /* Reread text file cache */
void do_say(CommandInvocation *invocation);       /* Messages to all. */
void do_search(CommandInvocation *invocation);    /* Search matching objects. */
void do_flag(CommandInvocation *invocation);
void do_setattr(CommandInvocation *invocation); /* Set object attribute */
void do_stats(CommandInvocation *invocation);   /* Display object statistics. */
void do_teleport(CommandInvocation *invocation);
void do_unlink(CommandInvocation *invocation);
void do_use(CommandInvocation *invocation);     /* Use object. */
void do_version(CommandInvocation *invocation); /* List MUX version number */
void do_wait(CommandInvocation *invocation);    /* Perform command after wait */
void do_session(CommandInvocation *invocation); /* Wizard session listing */
void do_telnet(CommandInvocation *invocation);  /* Wizard Telnet diagnostics */
void do_who(CommandInvocation *invocation);     /* Wizard WHO listing */
/* from log.c */
void do_log(CommandInvocation *invocation); /* Log to arbitrary logfile */

/* Mecha stuff */
void do_show(CommandInvocation *invocation);
