/* eval.h - Command evaluation, argument parsing, and evaluation-cache
 * interface. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/database/db.h"

typedef struct ServerConfiguration ServerConfiguration;

char *parse_to(const ServerConfiguration *configuration, char **string,
               char delimiter, int eval);
char *parse_arglist(EvaluationContext *context, DbRef player, DbRef cause,
                    char *string, char delimiter, long eval, char *arguments[],
                    DbRef max_arguments, char *commands[], DbRef command_count);
void exec(EvaluationContext *context, char *buffer, char **buffer_pointer,
          int flags, DbRef player, DbRef cause, int eval, char **commands,
          char *arguments[], int argument_count);
