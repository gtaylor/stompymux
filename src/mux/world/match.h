/** @file
 * Object-name matching state and matching operation declarations.
 */
#pragma once

#include <stdbool.h>

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/world/access.h"

typedef struct ServerConfiguration ServerConfiguration;

/* Match functions
 * Usage:
 *	init_match(player, name, type);
 *	match_this();
 *	match_that();
 *	...
 *	thing = match_result()
 */

/** Executes init match. @param[in,out] match_context Match context. @param[in]
 * player Player object. @param[in] name Name to use. @param[in] type Type. */

extern void init_match(MatchContext *match_context, DbRef player,
                       const char *name, int type);
/** Executes init match check keys. @param[in,out] match_context Match context.
 * @param[in] player Player object. @param[in] name Name to use. @param[in] type
 * Type. */

extern void init_match_check_keys(MatchContext *match_context, DbRef player,
                                  const char *name, int type);
/** Executes match player. @param[in] match_context Match context. */

extern void match_player(MatchContext *match_context);
/** Executes match absolute. @param[in] match_context Match context. */

extern void match_absolute(MatchContext *match_context);
/** Executes match numeric. @param[in] match_context Match context. */

extern void match_numeric(MatchContext *match_context);
/** Executes match me. @param[in] match_context Match context. */

extern void match_me(MatchContext *match_context);
/** Executes match here. @param[in] match_context Match context. */

extern void match_here(MatchContext *match_context);
/** Executes match possession. @param[in] match_context Match context. */

extern void match_possession(MatchContext *match_context);
/** Executes match neighbor. @param[in] match_context Match context. */

extern void match_neighbor(MatchContext *match_context);
/** Executes matches exit from list. @param[in] string String to process.
 * @param[in] pattern Pattern. */

bool matches_exit_from_list(const char *string, const char *pattern);
/** Executes match exit. @param[in] match_context Match context. */

extern void match_exit(MatchContext *match_context);
/** Executes match carried exit. @param[in] match_context Match context. */

extern void match_carried_exit(MatchContext *match_context);
/** Executes match everything. @param[in] match_context Match context.
 * @param[in] key Lookup key or command flags. */

extern void match_everything(MatchContext *match_context, int key);
/** Executes match result. @param[in] match_context Match context. */

extern DbRef match_result(MatchContext *match_context);
/** Executes last match result. @param[in,out] match_context Match context. */

extern DbRef last_match_result(MatchContext *match_context);
/** Executes match status. @param[in] evaluation Expression evaluation context.
 * @param[in] player Player object. @param[in] match Match. */

extern DbRef match_status(EvaluationContext *evaluation, DbRef player,
                          DbRef match);
/** Executes noisy match result. @param[in,out] match_context Match context. */

extern DbRef noisy_match_result(MatchContext *match_context);
/** Executes match zone exit. @param[in] match_context Match context. */

extern void match_zone_exit(MatchContext *match_context);
/** Executes match possessed. @param[in] match_context Match context. @param[in]
 * player Player object. @param[in] thing Thing. @param[in] target Target object
 * or value. @param[in] dflt Dflt. */

extern DbRef match_possessed(MatchContext *match_context, DbRef player,
                             DbRef thing, char *target, DbRef dflt);
/** Parses range. @param[in] database Game database. @param[in] configuration
 * Server configuration. @param[in,out] name Name to use. @param[in] low_bound
 * Low bound. @param[in] high_bound High bound. */

extern void parse_range(GameDatabase *database,
                        const ServerConfiguration *configuration, char **name,
                        DbRef *low_bound, DbRef *high_bound);
/** Parses thing slash. @param[in] match_context Match context. @param[in]
 * player Player object. @param[in] thing Thing. @param[in,out] after After.
 * @param[in] it It. */

extern bool parse_thing_slash(MatchContext *match_context, DbRef player,
                              char *thing, char **after, DbRef *it);

#define NOMATCH_MESSAGE "I don't see that here."
#define AMBIGUOUS_MESSAGE "I don't know which one you mean!"
#define NOPERM_MESSAGE "Permission denied."

enum : int {
  MAT_NO_EXITS = 1, /* Don't check for exits */
  MAT_NUMERIC = 4,  /* Check for un-#ified dbrefs */
};
