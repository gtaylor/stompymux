/** @file
 * Literal native-command argument parsing.
 */
#pragma once

#include <stddef.h>

#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

enum CommandParseFlags : int {
  COMMAND_PARSE_STRIP = 1 << 0,
  COMMAND_PARSE_STRIP_TRAILING = 1 << 1,
  COMMAND_PARSE_STRIP_LEADING = 1 << 2,
  COMMAND_PARSE_STRIP_AROUND = 1 << 3,
  COMMAND_PARSE_NO_COMPRESS = 1 << 4,
};

typedef struct CommandParseRequest {
  const ServerConfiguration *configuration;
  char **source;
  char delimiter;
  int options;
} CommandParseRequest;

typedef struct CommandArgumentListRequest {
  const ServerConfiguration *configuration;
  char *source;
  char delimiter;
  int options;
  char **arguments;
  size_t maximum_arguments;
} CommandArgumentListRequest;

/** Parses to. @param[in] request Request. */

char *parse_to(const CommandParseRequest *request);
/** Parses arglist. @param[in] request Request. */

void parse_arglist(const CommandArgumentListRequest *request);
