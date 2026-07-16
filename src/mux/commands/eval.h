/* eval.h - Command evaluation, argument parsing, and evaluation-cache
 * interface. */

#pragma once

#include "mux/database/db.h"

void tcache_init(void);
char *parse_to(char **string, char delimiter, int eval);
char *parse_arglist(DbRef player, DbRef cause, char *string, char delimiter,
                    long eval, char *arguments[], long max_arguments,
                    char *commands[], long command_count);
void exec(char *buffer, char **buffer_pointer, int flags, DbRef player,
          DbRef cause, int eval, char **commands, char *arguments[],
          int argument_count);
