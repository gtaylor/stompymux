/* command_helpers.h - Shared helpers for native command implementations. */

#pragma once

#include "mux/commands/command_context.h"

char *trim_space_sep(char *string, char separator);
char *next_token(char *string, char separator);
DbRef match_thing(MatchContext *match, DbRef player, char *name);
bool argument_count_in_range(const char *name, int count, int minimum,
                             int maximum, char *result, char **result_cursor);
char *get_uptime_to_string(int uptime);
int xlate(char *argument);
