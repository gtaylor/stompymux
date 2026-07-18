/* match.h - Object-name matching state and matching operation declarations. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/database/db.h"

typedef MatchContext MSTATE;
typedef struct ServerConfiguration ServerConfiguration;

/* Match functions
 * Usage:
 *	init_match(player, name, type);
 *	match_this();
 *	match_that();
 *	...
 *	thing = match_result()
 */

extern void init_match(MatchContext *, DbRef, char *, int);
extern void init_match_check_keys(MatchContext *, DbRef, char *, int);
extern void match_player(MatchContext *);
extern void match_absolute(MatchContext *);
extern void match_numeric(MatchContext *);
extern void match_me(MatchContext *);
extern void match_here(MatchContext *);
extern void match_home(MatchContext *);
extern void match_possession(MatchContext *);
extern void match_neighbor(MatchContext *);
extern void match_exit(MatchContext *);
extern void match_exit_with_parents(MatchContext *);
extern void match_carried_exit(MatchContext *);
extern void match_carried_exit_with_parents(MatchContext *);
extern void match_master_exit(MatchContext *);
extern void match_everything(MatchContext *, int);
extern DbRef match_result(MatchContext *);
extern DbRef last_match_result(MatchContext *);
extern DbRef match_status(EvaluationContext *, DbRef, DbRef);
extern DbRef noisy_match_result(MatchContext *);
extern void save_match_state(MatchContext *, MSTATE *);
extern void restore_match_state(MatchContext *, MSTATE *);
extern void match_zone_exit(MatchContext *);
extern DbRef match_possessed(MatchContext *, DbRef player, DbRef thing,
                             char *target, DbRef default_match,
                             int check_enter);
extern void parse_range(GameDatabase *database,
                        const ServerConfiguration *configuration, char **name,
                        DbRef *low_bound, DbRef *high_bound);
extern int parse_thing_slash(MatchContext *, DbRef player, char *thing,
                             char **after, DbRef *object);
extern int get_obj_and_lock(MatchContext *,
                            const ServerConfiguration *configuration,
                            DbRef player, char *what, DbRef *object,
                            Attribute **attribute, char *error_message,
                            char **buffer_pointer);

#define NOMATCH_MESSAGE "I don't see that here."
#define AMBIGUOUS_MESSAGE "I don't know which one you mean!"
#define NOPERM_MESSAGE "Permission denied."

enum : int {
  MAT_NO_EXITS = 1,     /* Don't check for exits */
  MAT_EXIT_PARENTS = 2, /* Check for exits in parents */
  MAT_NUMERIC = 4,      /* Check for un-#ified dbrefs */
  MAT_HOME = 8,         /* Check for 'home' */
};
