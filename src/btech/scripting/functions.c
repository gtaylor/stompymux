
/* Implements BattleTech scripting functions. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "command_handlers_api.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

char *btech_attribute_read(GameDatabase *database, DbRef id, int flag,
                           char buffer[static LBUF_SIZE]) {
  long flags;

  return attribute_get_string(database, buffer, id, flag, &flags);
}

void silly_atr_set_in(GameDatabase *database, DbRef id, int flag,
                      const char *data) {
  attribute_add_raw(database, id, flag, data);
}

static char **text_slot(char **lines, size_t count, size_t index) {
  return checked_storage_at(lines, count, sizeof(*lines), index);
}

void FreeTextItems(char **lines, size_t count) {
  for (size_t index = 0; index < count; ++index)
    free(*text_slot(lines, count, index));
}

void KillText(char **lines, size_t count) {
  FreeTextItems(lines, count);
  free(lines);
}

void ShowText(EvaluationContext *evaluation, char **lines, size_t count,
              DbRef player) {
  for (size_t index = 0; index < count; ++index)
    mecha_notify(evaluation, player, *text_slot(lines, count, index));
}

int BOUNDED(int min, int val, int max) {
  if (val < min)
    return min;
  if (val > max)
    return max;
  return val;
}

float FBOUNDED(float min, float val, float max) {
  if (val < min)
    return min;
  if (val > max)
    return max;
  return val;
}

int MAX(int v1, int v2) {
  if (v1 > v2)
    return v1;
  return v2;
}

int MIN(int v1, int v2) {
  if (v1 < v2)
    return v1;
  return v2;
}

/*
 * Gets the first parameter from a string
 * and returns it.
 */
char *first_parseattribute(char *buffer) {
  size_t length;
  char *start, *first;

  /* Look for the first parameter */
  start = buffer;
  length = strcspn(start, " \t=");

  /* If the first parameter is to big set the size */
  if (length > SBUF_SIZE)
    length = SBUF_SIZE;

  /* Make it and return it */
  first = strndup(start, length);

  return first;
}

/*
 * proper_parseattributes
 *
 * Split user input on whitespace delimeters, including '='.
 *
 *
 * This function is designed to be used for commands of the nature
 *
 * COMMAND KEY=VALUE
 *
 * It expects the null terminated source character buffer as the firs
 * argument, buffer.
 *
 * It will duplicate each, space, tab, or equal-sign delimeted field
 * and assign a pointer to it in the string pointer array, args, in
 * ascending order for up to max-1 fields.
 *
 * All remaining input, if any, will be duplicated and the address of
 * duplicate string will be assigned as the last entry in args.
 *
 * For the above example, the result would be a set of strings with
 * the contents:
 *
 * "COMMAND", "=", "KEY", "VALUE"
 *
 * NOTE: it is the caller's responsibility to free the duplicated
 * strings.
 */

int proper_parseattributes(char *buffer, char **args, int max) {
  if (max <= 0)
    return 0;
  const size_t argument_capacity = (size_t)max;
  memset(args, 0, sizeof(*args) * argument_capacity);

  size_t count = 0;
  size_t offset = 0;
  const size_t buffer_length = strlen(buffer);
  while (count < argument_capacity - 1 && offset < buffer_length) {
    const char *start = checked_string_suffix(buffer, offset);
    char **slot = text_slot(args, argument_capacity, count);
    if (*start == '=') {
      *slot = strndup(start, 1);
      ++count;
      ++offset;
      continue;
    }
    const size_t length = strcspn(start, " \t=");
    *slot = strndup(start, length);
    ++count;
    offset += length;
    const char current = *checked_string_suffix(buffer, offset);
    if (current != '=' && current != '\0')
      ++offset;
  }
  if (offset < buffer_length) {
    *text_slot(args, argument_capacity, argument_capacity - 1) =
        strdup(checked_string_suffix(buffer, offset));
    ++count;
  }
  return (int)count;
}

int silly_parseattributes(char *buffer, char **args, int max) {
  if (max <= 0)
    return 0;
  const size_t argument_capacity = (size_t)max;
  memset(args, 0, sizeof(*args) * argument_capacity);

  char expanded[LBUF_SIZE];
  size_t output = 0;
  const size_t input_length = strlen(buffer);
  for (size_t input = 0; input < input_length; ++input) {
    const char value = *checked_string_suffix(buffer, input);
    const size_t needed = value == '=' ? 3 : 1;
    if (output + needed >= sizeof(expanded))
      return 0;
    if (value == '=') {
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = ' ';
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = '=';
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = ' ';
    } else {
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = value;
    }
  }
  *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                              output) = '\0';

  char *save_pointer = nullptr;
  char *parsed = strtok_r(expanded, " \t", &save_pointer);
  size_t count = 0;
  while (parsed != nullptr && count < argument_capacity) {
    if (count == argument_capacity - 1) {
      char remainder[LBUF_SIZE];
      (void)snprintf(remainder, sizeof(remainder), "%s", parsed);
      while ((parsed = strtok_r(nullptr, " \t", &save_pointer)) != nullptr) {
        const size_t used = strlen(remainder);
        (void)snprintf(checked_storage_at(remainder, sizeof(remainder),
                                          sizeof(char), used),
                       sizeof(remainder) - used, " %s", parsed);
      }
      *text_slot(args, argument_capacity, count) = strdup(remainder);
    } else {
      *text_slot(args, argument_capacity, count) = strdup(parsed);
      parsed = strtok_r(nullptr, " \t", &save_pointer);
    }
    ++count;
  }
  return (int)count;
}

/*
 * proper_explodearguments
 *
 * Split user input on whitespace delimeters.
 *
 *
 * This function is designed to be used for commands of the nature
 *
 * COMMAND VALUE1 VALUE2 VALUE3
 *
 * It expects the null terminated source character buffer as the firs
 * argument, buffer.
 *
 * It will duplicate each space or tab delimeted field and assign a
 * pointer to it in the string pointer array, args, in ascending order
 * for up to max-1 fields.
 *
 * All remaining input, if any, will be duplicated and the address of
 * duplicate string will be assigned as the last entry in args.
 *
 * For the above example, the result would be a set of strings with
 * the contents:
 *
 * "COMMAND", "VALUE1", "VALUE2", "VALUE3"
 *
 * NOTE: it is the caller's responsibility to free the duplicated
 * strings.
 */

int proper_explodearguments(const char *buffer, char **args, int max) {
  int count = 0;

  if (max <= 0)
    return 0;
  const size_t argument_capacity = (size_t)max;
  memset(args, 0, sizeof(*args) * argument_capacity);

  char *storage = strdup(buffer);
  if (storage == nullptr)
    return 0;
  char *remaining = storage;
  while (count < max - 1 && remaining != nullptr && *remaining) {
    char *field = strsep(&remaining, " \t");
    char **slot = checked_storage_at(args, argument_capacity, sizeof(char *),
                                     (size_t)count);
    *slot = strdup(field);
    count++;
  }
  if (remaining != nullptr && *remaining) {
    char **slot = checked_storage_at(args, argument_capacity, sizeof(char *),
                                     argument_capacity - 1);
    *slot = strdup(remaining);
    count++;
  }
  free(storage);
  return count;
}

int mech_parseattributes(char *buffer, char **args, int maxargs) {
  int count = 0;
  char *parsed = buffer;
  int num_args = 0;

  if (maxargs <= 0)
    return 0;
  const size_t argument_capacity = (size_t)maxargs;
  memset(args, 0, sizeof(*args) * argument_capacity);

  while ((count < maxargs) && parsed) {
    parsed = strtok(!count ? buffer : NULL, " \t");
    *text_slot(args, argument_capacity, (size_t)count) = parsed;
    if (parsed)
      num_args++; /* Actual count of arguments */
    count++;      /* Loop to make sure we don't overrun our */
                  /* buffer */
  }
  return num_args;
}
