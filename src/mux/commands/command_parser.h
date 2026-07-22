/* command_parser.h - Literal native-command argument parsing. */

#pragma once

#include "mux/objects/db.h"

typedef struct ServerConfiguration ServerConfiguration;

enum CommandParseFlags {
  COMMAND_PARSE_STRIP = 1 << 0,
  COMMAND_PARSE_STRIP_TRAILING = 1 << 1,
  COMMAND_PARSE_STRIP_LEADING = 1 << 2,
  COMMAND_PARSE_STRIP_AROUND = 1 << 3,
  COMMAND_PARSE_NO_COMPRESS = 1 << 4,
};

char *parse_to(const ServerConfiguration *configuration, char **string,
               char delimiter, int flags);
char *parse_arglist(const ServerConfiguration *configuration, char *string,
                    char delimiter, int flags, char *arguments[],
                    DbRef max_arguments);
