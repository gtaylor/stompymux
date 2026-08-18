/** @file
 * Shared helpers for native command implementations.
 */
#pragma once

#include "mux/commands/command_context.h"
#include "mux/server/platform.h"
#include "mux/support/owned_text.h"

/** Executes trim space sep. @param[in,out] string String to process. @param[in]
 * separator Separator. */

char *trim_space_sep(char *string, char separator);
/** Executes next token. @param[in,out] string String to process. @param[in]
 * separator Separator. */

char *next_token(char *string, char separator);
/** Executes match thing. @param[in] match Match. @param[in] player Player
 * object. @param[in] name Name to use. */

DbRef match_thing(MatchContext *match, DbRef player, char *name);
/** Executes argument count in range. @param[in] name Name to use. @param[in]
 * count Number of elements. @param[in] minimum Minimum. @param[in] maximum
 * Maximum. @param[out] result Result. @param[in,out] result_cursor Result
 * cursor. */

bool argument_count_in_range(const char *name, int count, int minimum,
                             int maximum, char *result, char **result_cursor);
/** Returns uptime to string. @param[in] uptime Uptime. */

OwnedText get_uptime_to_string(int uptime);
/** Executes xlate. @param[in,out] argument Command argument. */

int xlate(char *argument);
