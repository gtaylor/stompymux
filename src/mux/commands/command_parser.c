/* command_parser.c - Literal native-command argument parsing. */

#include "mux/commands/command_parser.h"

#include <ctype.h>

#include "mux/server/server_config.h"
#include "mux/support/alloc.h"

static char *parse_cleanup(const ServerConfiguration *configuration, int flags,
                           bool first, char *result, char *cursor) {
  if ((configuration->space_compress ||
       (flags & COMMAND_PARSE_STRIP_TRAILING)) &&
      !(flags & COMMAND_PARSE_NO_COMPRESS) && !first && cursor[-1] == ' ')
    cursor--;

  if ((flags & COMMAND_PARSE_STRIP_AROUND) && result[0] == '{' &&
      cursor > result && cursor[-1] == '}') {
    result++;
    if (configuration->space_compress &&
        (!(flags & COMMAND_PARSE_NO_COMPRESS) ||
         (flags & COMMAND_PARSE_STRIP_LEADING)))
      while (*result && isspace((unsigned char)*result))
        result++;
    cursor--;
    while (cursor > result && isspace((unsigned char)cursor[-1]))
      cursor--;
  }
  *cursor = '\0';
  return result;
}

char *parse_to(const ServerConfiguration *configuration, char **source,
               char delimiter, int flags) {
  if (source == nullptr || *source == nullptr)
    return nullptr;
  if (**source == '\0') {
    char *empty = *source;
    *source = nullptr;
    return empty;
  }

  char *result = *source;
  if ((configuration->space_compress ||
       (flags & COMMAND_PARSE_STRIP_LEADING)) &&
      !(flags & COMMAND_PARSE_NO_COMPRESS)) {
    while (*result && isspace((unsigned char)*result))
      result++;
  }

  char stack[32];
  size_t depth = 0;
  int brace_depth = 0;
  bool first = true;
  char *read = result;
  char *write = result;
  while (*read) {
    if (*read == '\\' && read[1]) {
      *write++ = *read++;
      *write++ = *read++;
      first = false;
      continue;
    }

    if (*read == '{') {
      brace_depth++;
      if (!(flags & COMMAND_PARSE_STRIP) || brace_depth > 1)
        *write++ = *read;
      read++;
      first = false;
      continue;
    }
    if (*read == '}' && brace_depth > 0) {
      brace_depth--;
      if (!(flags & COMMAND_PARSE_STRIP) || brace_depth > 0)
        *write++ = *read;
      read++;
      first = false;
      continue;
    }

    if (brace_depth == 0 && (*read == '(' || *read == '[')) {
      if (depth < sizeof(stack))
        stack[depth++] = *read == '(' ? ')' : ']';
    } else if (brace_depth == 0 && depth > 0 && *read == stack[depth - 1]) {
      depth--;
    } else if (brace_depth == 0 && depth == 0 && *read == delimiter) {
      char *after = read + 1;
      result = parse_cleanup(configuration, flags, first, result, write);
      *source = after;
      return result;
    }

    if (*read == ' ' && configuration->space_compress &&
        !(flags & COMMAND_PARSE_NO_COMPRESS)) {
      if (first) {
        read++;
        result++;
        continue;
      }
      if (write > result && write[-1] == ' ') {
        read++;
        continue;
      }
    }
    first = false;
    *write++ = *read++;
  }

  result = parse_cleanup(configuration, flags, first, result, write);
  *source = nullptr;
  return result;
}

char *parse_arglist(const ServerConfiguration *configuration, char *string,
                    char delimiter, int flags, char *arguments[],
                    DbRef max_arguments) {
  for (DbRef i = 0; i < max_arguments; i++)
    arguments[i] = nullptr;
  if (string == nullptr)
    return nullptr;

  char *remainder = string;
  char *list = parse_to(configuration, &remainder, delimiter, 0);
  for (DbRef i = 0; i < max_arguments && list != nullptr; i++) {
    char separator = i < max_arguments - 1 ? ',' : '\0';
    char *argument = parse_to(configuration, &list, separator, flags);
    arguments[i] = alloc_lbuf("parse_arglist");
    StringCopy(arguments[i], argument);
  }
  return remainder;
}
