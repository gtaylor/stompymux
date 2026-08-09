/* command_parser.c - Literal native-command argument parsing. */

#include "mux/commands/command_parser.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h" // IWYU pragma: keep

#include <ctype.h>
#include <stddef.h>
#include <string.h>

#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

static char *parse_cleanup(const ServerConfiguration *configuration, int flags,
                           bool first, char *text, size_t capacity,
                           size_t result_offset, size_t cursor_offset) {
  if ((configuration->space_compress ||
       (flags & COMMAND_PARSE_STRIP_TRAILING)) &&
      !(flags & COMMAND_PARSE_NO_COMPRESS) && !first && cursor_offset > 0 &&
      *(const char *)checked_storage_at_const(text, capacity, sizeof(char),
                                              cursor_offset - 1) == ' ')
    cursor_offset--;

  if ((flags & COMMAND_PARSE_STRIP_AROUND) &&
      *(const char *)checked_storage_at_const(text, capacity, sizeof(char),
                                              result_offset) == '{' &&
      cursor_offset > result_offset &&
      *(const char *)checked_storage_at_const(text, capacity, sizeof(char),
                                              cursor_offset - 1) == '}') {
    result_offset++;
    if (configuration->space_compress &&
        (!(flags & COMMAND_PARSE_NO_COMPRESS) ||
         (flags & COMMAND_PARSE_STRIP_LEADING)))
      while (*(const char *)checked_storage_at_const(
                 text, capacity, sizeof(char), result_offset) != '\0' &&
             (isspace)((unsigned char)*(const char *)checked_storage_at_const(
                 text, capacity, sizeof(char), result_offset)))
        result_offset++;
    cursor_offset--;
    while (cursor_offset > result_offset &&
           (isspace)((unsigned char)*(const char *)checked_storage_at_const(
               text, capacity, sizeof(char), cursor_offset - 1)))
      cursor_offset--;
  }
  *(char *)checked_storage_at(text, capacity, sizeof(char), cursor_offset) =
      '\0';
  return checked_mutable_string_suffix(text, result_offset);
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

  char *text = *source;
  const size_t length = strlen(text);
  const size_t capacity = length + 1;
  size_t result_offset = 0;
  if ((configuration->space_compress ||
       (flags & COMMAND_PARSE_STRIP_LEADING)) &&
      !(flags & COMMAND_PARSE_NO_COMPRESS)) {
    while (result_offset < length &&
           (isspace)((unsigned char)*(const char *)checked_storage_at_const(
               text, capacity, sizeof(char), result_offset)))
      result_offset++;
  }

  char stack[32];
  size_t depth = 0;
  int brace_depth = 0;
  bool first = true;
  size_t read_offset = result_offset;
  size_t write_offset = result_offset;
  while (read_offset < length) {
    char character = *(const char *)checked_storage_at_const(
        text, capacity, sizeof(char), read_offset);

    if (character == '\\' && read_offset + 1 < length) {
      *(char *)checked_storage_at(text, capacity, sizeof(char),
                                  write_offset++) = character;
      read_offset++;
      *(char *)checked_storage_at(text, capacity, sizeof(char),
                                  write_offset++) =
          *(const char *)checked_storage_at_const(text, capacity, sizeof(char),
                                                  read_offset++);
      first = false;
      continue;
    }

    if (character == '{') {
      brace_depth++;
      if (!(flags & COMMAND_PARSE_STRIP) || brace_depth > 1)
        *(char *)checked_storage_at(text, capacity, sizeof(char),
                                    write_offset++) = character;
      read_offset++;
      first = false;
      continue;
    }
    if (character == '}' && brace_depth > 0) {
      brace_depth--;
      if (!(flags & COMMAND_PARSE_STRIP) || brace_depth > 0)
        *(char *)checked_storage_at(text, capacity, sizeof(char),
                                    write_offset++) = character;
      read_offset++;
      first = false;
      continue;
    }

    if (brace_depth == 0 && (character == '(' || character == '[')) {
      if (depth < sizeof(stack))
        *(char *)checked_storage_at(stack, sizeof(stack), sizeof(char),
                                    depth++) = character == '(' ? ')' : ']';
    } else if (brace_depth == 0 && depth > 0 &&
               character ==
                   *(const char *)checked_storage_at_const(
                       stack, sizeof(stack), sizeof(char), depth - 1)) {
      depth--;
    } else if (brace_depth == 0 && depth == 0 && character == delimiter) {
      char *result = parse_cleanup(configuration, flags, first, text, capacity,
                                   result_offset, write_offset);

      *source =
          checked_storage_at(text, capacity, sizeof(char), read_offset + 1);
      return result;
    }

    if (character == ' ' && configuration->space_compress &&
        !(flags & COMMAND_PARSE_NO_COMPRESS)) {
      if (first) {
        read_offset++;
        result_offset++;
        continue;
      }
      if (write_offset > result_offset &&
          *(const char *)checked_storage_at_const(text, capacity, sizeof(char),
                                                  write_offset - 1) == ' ') {
        read_offset++;
        continue;
      }
    }
    first = false;
    *(char *)checked_storage_at(text, capacity, sizeof(char), write_offset++) =
        character;
    read_offset++;
  }

  char *result = parse_cleanup(configuration, flags, first, text, capacity,
                               result_offset, write_offset);
  *source = nullptr;
  return result;
}

char *parse_arglist(const ServerConfiguration *configuration, char *string,
                    char delimiter, int flags, char *arguments[],
                    DbRef max_arguments) {
  for (DbRef i = 0; i < max_arguments; i++)
    *(char **)checked_storage_at(arguments, (size_t)max_arguments,
                                 sizeof(*arguments), (size_t)i) = nullptr;
  if (string == nullptr)
    return nullptr;

  char *remainder = string;
  char *list = parse_to(configuration, &remainder, delimiter, 0);
  for (DbRef i = 0; i < max_arguments && list != nullptr; i++) {
    char separator = i < max_arguments - 1 ? ',' : '\0';
    char *argument = parse_to(configuration, &list, separator, flags);
    char **slot = checked_storage_at(arguments, (size_t)max_arguments,
                                     sizeof(*arguments), (size_t)i);

    *slot = alloc_lbuf("parse_arglist");
    StringCopy(*slot, argument);
  }
  return remainder;
}
