/* match.h - Object-name matching state and matching operation declarations. */

#pragma once

#include <stdbool.h>

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/world/access.h"

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

extern void init_match(MatchContext * /*match_context*/, DbRef /*player*/,
                       const char * /*name*/, int /*type*/);
extern void init_match_check_keys(MatchContext * /*match_context*/,
                                  DbRef /*player*/, const char * /*name*/,
                                  int /*type*/);
extern void match_player(MatchContext * /*match_context*/);
extern void match_absolute(MatchContext * /*match_context*/);
extern void match_numeric(MatchContext * /*match_context*/);
extern void match_me(MatchContext * /*match_context*/);
extern void match_here(MatchContext * /*match_context*/);
extern void match_home(MatchContext * /*match_context*/);
extern void match_possession(MatchContext * /*match_context*/);
extern void match_neighbor(MatchContext * /*match_context*/);
bool matches_exit_from_list(const char *string, const char *pattern);
extern void match_exit(MatchContext * /*match_context*/);
extern void match_carried_exit(MatchContext * /*match_context*/);
extern void match_everything(MatchContext * /*match_context*/, int /*key*/);
extern DbRef match_result(MatchContext * /*match_context*/);
extern DbRef last_match_result(MatchContext * /*match_context*/);
extern DbRef match_status(EvaluationContext * /*evaluation*/, DbRef /*player*/,
                          DbRef /*match*/);
extern DbRef noisy_match_result(MatchContext * /*match_context*/);
extern void save_match_state(MatchContext * /*match_context*/,
                             MSTATE * /*mstate*/);
extern void restore_match_state(MatchContext * /*match_context*/,
                                MSTATE * /*mstate*/);
extern void match_zone_exit(MatchContext * /*match_context*/);
extern DbRef match_possessed(MatchContext * /*match_context*/, DbRef player,
                             DbRef thing, char *target, DbRef dflt);
extern void parse_range(GameDatabase *database,
                        const ServerConfiguration *configuration, char **name,
                        DbRef *low_bound, DbRef *high_bound);
extern bool parse_thing_slash(MatchContext * /*match_context*/, DbRef player,
                              char *thing, char **after, DbRef *it);

#define NOMATCH_MESSAGE "I don't see that here."
#define AMBIGUOUS_MESSAGE "I don't know which one you mean!"
#define NOPERM_MESSAGE "Permission denied."

enum : int {
  MAT_NO_EXITS = 1, /* Don't check for exits */
  MAT_NUMERIC = 4,  /* Check for un-#ified dbrefs */
  MAT_HOME = 8,     /* Check for 'home' */
};
