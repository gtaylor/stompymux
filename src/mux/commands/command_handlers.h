/** @file
 * Native command handler declarations used by the command table.
 */
#pragma once

#include "mux/commands/command_invocation.h"
#include "mux/objects/db.h"

typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

/* from comsys.c */

/** Handles the checkchannel command. @param[in] player Player object.
 * @param[in] cause Object that caused the operation. @param[in] key Lookup key
 * or command flags. @param[in,out] channel Channel. */

void do_checkchannel(DbRef player, DbRef cause, int key,
                     char *channel); /* check a channel */
/** Handles the edituser command. @param[in] player Player object. @param[in]
 * cause Object that caused the operation. @param[in] key Lookup key or command
 * flags. @param[in,out] channel Channel. @param[in,out] user User. */

void do_edituser(DbRef player, DbRef cause, int key, char *channel,
                 char *user); /* edit a channel user */
/** Handles the tapcom command. @param[in] player Player object. @param[in]
 * cause Object that caused the operation. @param[in] key Lookup key or command
 * flags. @param[in,out] channel Channel. @param[in,out] target Target object or
 * value. */

void do_tapcom(DbRef player, DbRef cause, int key, char *channel,
               char *target); /* taps a channel */

/** Handles the admin command. @param[in,out] invocation Command invocation. */

void do_admin(CommandInvocation *invocation); /* Change config parameters */
/** Handles typed BTech special-object registration and inspection. */
void do_btech(CommandInvocation *invocation);
/** Applies a wizard administration command to the unit containing the player.
 * @param[in,out] invocation Command invocation. */
void do_mech_admin(CommandInvocation *invocation);
/** Handles the alias command. @param[in,out] invocation Command invocation. */

void do_alias(CommandInvocation *invocation);
/** Handles the boot command. @param[in,out] invocation Command invocation. */

void do_boot(CommandInvocation *invocation);
/** Handles the chzone command. @param[in,out] invocation Command invocation. */

void do_chzone(CommandInvocation *invocation);
/** Handles the color command. @param[in,out] invocation Command invocation. */

void do_color(CommandInvocation *invocation);
/** Handles the clone command. @param[in,out] invocation Command invocation. */

void do_clone(CommandInvocation *invocation);
/** Handles the create command. @param[in] invocation Command invocation. */

void do_create(CommandInvocation *invocation);
/** Handles the destroy command. @param[in,out] invocation Command invocation.
 */

void do_destroy(CommandInvocation *invocation);
/** Handles object description commands. */

void do_description(CommandInvocation *invocation);
/** Handles the dig command. @param[in,out] invocation Command invocation. */

void do_dig(CommandInvocation *invocation);
/** Handles the dump command. @param[in,out] invocation Command invocation. */

void do_dump(CommandInvocation *invocation); /* Dump the database */
/** Handles the find command. @param[in] invocation Command invocation. */

void do_find(CommandInvocation *invocation); /* Search the database. */
/** Handles the force command. @param[in,out] invocation Command invocation. */

void do_force(CommandInvocation *invocation);
/** Handles the force prefixed command. @param[in,out] invocation Command
 * invocation. */

void do_force_prefixed(CommandInvocation *invocation); /* #num cmd FORCE */
/** Handles the give command. @param[in,out] invocation Command invocation. */

void do_give(CommandInvocation *invocation); /* Give something away. */
/** Handles the global command. @param[in,out] invocation Command invocation. */

void do_global(CommandInvocation *invocation);
/** Executes list global controls. @param[in] evaluation Expression evaluation
 * context. @param[in] configuration Server configuration. @param[in] player
 * Player object. */

void list_global_controls(EvaluationContext *evaluation,
                          ServerConfiguration *configuration, DbRef player);
/** Handles the halt command. @param[in,out] invocation Command invocation. */

void do_halt(CommandInvocation *invocation); /* Remove commands from queue */
/** Handles the history command. @param[in] player Player object. @param[in]
 * cause Object that caused the operation. @param[in] key Lookup key or command
 * flags. @param[in,out] argument Command argument. */

void do_history(DbRef player, DbRef cause, int key,
                char *argument); /* View various history info */
/** Handles the multis command. @param[in] player Player object. @param[in]
 * cause Object that caused the operation. @param[in] key Lookup key or command
 * flags. */

void do_multis(DbRef player, DbRef cause, int key);
/** Handles the link command. @param[in,out] invocation Command invocation. */

void do_link(CommandInvocation *invocation);
/** Handles the lua command. @param[in,out] invocation Command invocation. */

void do_lua(CommandInvocation *invocation);
/** Handles the list command. @param[in,out] invocation Command invocation. */

void do_list(CommandInvocation *invocation); /* List internal tables. */
/** Handles the look command. @param[in,out] invocation Command invocation. */

void do_look(CommandInvocation *invocation); /* Look here or at something. */
/** Handles the mudwho command. @param[in] player Player object. @param[in]
 * cause Object that caused the operation. @param[in] key Lookup key or command
 * flags. @param[in,out] name Name to use. @param[in,out] command Command text
 * or descriptor. */

void do_mudwho(DbRef player, DbRef cause, int key, char *name,
               char *command); /* WHO for inter-mud page/who suppt */
/** Handles the name command. @param[in] invocation Command invocation. */

void do_name(CommandInvocation *invocation);
/** Handles the newpassword command. @param[in,out] invocation Command
 * invocation. */

void do_newpassword(CommandInvocation *invocation);
/** Handles the open command. @param[in,out] invocation Command invocation. */

void do_open(CommandInvocation *invocation);
/** Handles the page command. @param[in,out] invocation Command invocation. */

void do_page(CommandInvocation *invocation); /* Message a faraway player. */
/** Handles the pcreate command. @param[in] invocation Command invocation. */

void do_pcreate(CommandInvocation *invocation);
/** Handles the pemit command. @param[in,out] invocation Command invocation. */

void do_pemit(CommandInvocation *invocation); /* Message a specific object. */
/** Handles the power command. @param[in,out] invocation Command invocation. */

void do_power(CommandInvocation *invocation); /* Sets powers */
/** Handles the quit command. @param[in,out] invocation Command invocation. */

void do_quit(CommandInvocation *invocation); /* Disconnect this session */
/** Handles the readcache command. @param[in,out] invocation Command invocation.
 */

void do_readcache(CommandInvocation *invocation); /* Reread text file cache */
/** Handles the say command. @param[in,out] invocation Command invocation. */

void do_say(CommandInvocation *invocation); /* Messages to all. */
/** Handles the search command. @param[in,out] invocation Command invocation. */

void do_search(CommandInvocation *invocation); /* Search matching objects. */
/** Handles the flag command. @param[in,out] invocation Command invocation. */

void do_flag(CommandInvocation *invocation);
/** Handles the stats command. @param[in,out] invocation Command invocation. */

void do_stats(CommandInvocation *invocation); /* Display object statistics. */
/** Handles the teleport command. @param[in,out] invocation Command invocation.
 */

void do_teleport(CommandInvocation *invocation);
/** Handles the unlink command. @param[in,out] invocation Command invocation. */

void do_unlink(CommandInvocation *invocation);
/** Handles the use command. @param[in,out] invocation Command invocation. */

void do_use(CommandInvocation *invocation); /* Use object. */
/** Handles the version command. @param[in,out] invocation Command invocation.
 */

void do_version(CommandInvocation *invocation); /* List MUX version number */
/** Handles the wait command. @param[in,out] invocation Command invocation. */

void do_wait(CommandInvocation *invocation); /* Perform command after wait */
/** Handles the session command. @param[in,out] invocation Command invocation.
 */

void do_session(CommandInvocation *invocation); /* Wizard session listing */
/** Handles the telnet command. @param[in,out] invocation Command invocation. */

void do_telnet(CommandInvocation *invocation); /* Wizard Telnet diagnostics */
/** Handles the who command. @param[in,out] invocation Command invocation. */

void do_who(CommandInvocation *invocation); /* Wizard WHO listing */
/* from log.c */
/** Handles the log command. @param[in,out] invocation Command invocation. */

void do_log(CommandInvocation *invocation); /* Log to arbitrary logfile */

/* Mecha stuff */
/** Handles the show command. @param[in,out] invocation Command invocation. */

void do_show(CommandInvocation *invocation);
