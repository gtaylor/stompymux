
/* Implements BattleTech scripting functions. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "command_handlers_api.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "script_functions_api.h"

static bool btech_script_output_is_error(const char *output) {
  return !strncmp(output, "#-", 2) || !strcmp(output, "?");
}

static bool btech_script_list_separator(char character) {
  return character == ' ' || character == '|';
}

static size_t btech_script_list_item_count(const char *output) {
  size_t count = 0;
  bool in_item = false;
  const size_t LENGTH = strlen(output);

  for (size_t offset = 0; offset < LENGTH; offset++) {
    const char CHARACTER = *checked_string_suffix(output, offset);
    if (btech_script_list_separator(CHARACTER)) {
      in_item = false;
    } else if (!in_item) {
      count++;
      in_item = true;
    }
  }
  return count;
}

static BtechScriptList btech_script_list_parse(char *output) {
  BtechScriptList list = {0};
  list.count = btech_script_list_item_count(output);
  if (list.count == 0)
    return list;
  list.items = calloc(list.count, sizeof(*list.items));
  if (list.items == nullptr) {
    list.count = 0;
    return list;
  }

  const size_t LENGTH = strlen(output);
  size_t offset = 0;
  for (size_t index = 0; index < list.count; index++) {
    while (offset < LENGTH &&
           btech_script_list_separator(*checked_string_suffix(output, offset)))
      offset++;
    char *token =
        checked_storage_at(output, LENGTH + 1, sizeof(*output), offset);
    while (offset < LENGTH &&
           !btech_script_list_separator(*checked_string_suffix(output, offset)))
      offset++;
    if (offset < LENGTH) {
      *(char *)checked_storage_at(output, LENGTH + 1, sizeof(*output), offset) =
          '\0';
      offset++;
    }

    const char *number_text =
        *token == '#' ? checked_string_suffix(token, 1) : token;
    long number = 0;
    BtechScriptListItem *item =
        checked_storage_at(list.items, list.count, sizeof(*list.items), index);
    if (parse_long_checked(number_text, &number)) {
      item->kind = BTECH_SCRIPT_LIST_NUMBER;
      item->value.number = number;
    } else {
      item->kind = BTECH_SCRIPT_LIST_TEXT;
      item->value.text = token;
    }
  }
  return list;
}

BtechScriptResult btech_script_result_finish(BtechScriptCall *call,
                                             BtechScriptValueKind kind) {
  *call->output.cursor = '\0';
  BtechScriptResult result = {.status = BTECH_SCRIPT_OK, .kind = kind};
  if (btech_script_output_is_error(call->output.buffer)) {
    result.status = BTECH_SCRIPT_ERROR;
    result.kind = BTECH_SCRIPT_TEXT;
    result.value.text = call->output.buffer;
    return result;
  }

  switch (kind) {
  case BTECH_SCRIPT_TEXT:
    result.value.text = call->output.buffer;
    break;
  case BTECH_SCRIPT_LIST:
    result.value.list = btech_script_list_parse(call->output.buffer);
    break;
  case BTECH_SCRIPT_NUMBER:
    result.value.number = strtod(call->output.buffer, nullptr);
    break;
  case BTECH_SCRIPT_BOOLEAN:
    result.value.boolean = strcmp(call->output.buffer, "0") != 0;
    break;
  case BTECH_SCRIPT_MUTATION:
    result.value.mutation = true;
    break;
  }
  return result;
}

void btech_script_result_destroy(BtechScriptResult *result) {
  if (result->status == BTECH_SCRIPT_OK && result->kind == BTECH_SCRIPT_LIST)
    free(result->value.list.items);
  *result = (BtechScriptResult){0};
}

char *btech_attribute_read(GameDatabase *database, DbRef id, int flag,
                           char buffer[static LBUF_SIZE]) {
  long flags;

  return attribute_get_string(database, id, flag, buffer, LBUF_SIZE, &flags);
}

void silly_atr_set_in(GameDatabase *database, DbRef id, int flag,
                      const char *data) {
  attribute_add_raw(database, id, flag, data);
}

static char **text_slot(char **lines, size_t count, size_t index) {
  return (char **)checked_storage_at((void *)lines, count, sizeof(*lines),
                                     index);
}

void free_text_items(char **lines, size_t count) {
  for (size_t index = 0; index < count; ++index)
    free(*text_slot(lines, count, index));
}

void kill_text(char **lines, size_t count) {
  free_text_items(lines, count);
  free((void *)lines);
}

void show_text(EvaluationContext *evaluation, char **lines, size_t count,
               DbRef player) {
  for (size_t index = 0; index < count; ++index)
    mecha_notify(evaluation, player, *text_slot(lines, count, index));
}

int bounded(int min, int val, int max) {
  if (val < min)
    return min;
  if (val > max)
    return max;
  return val;
}

float fbounded(float min, float val, float max) {
  if (val < min)
    return min;
  if (val > max)
    return max;
  return val;
}

int max(int v1, int v2) {
  if (v1 > v2)
    return v1;
  return v2;
}

int min(int v1, int v2) {
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
  char *start;
  char *first;

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
  const size_t ARGUMENT_CAPACITY = (size_t)max;
  memset((void *)args, 0, sizeof(*args) * ARGUMENT_CAPACITY);

  size_t count = 0;
  size_t offset = 0;
  const size_t BUFFER_LENGTH = strlen(buffer);
  while (count < ARGUMENT_CAPACITY - 1 && offset < BUFFER_LENGTH) {
    const char *start = checked_string_suffix(buffer, offset);
    char **slot = text_slot(args, ARGUMENT_CAPACITY, count);
    if (*start == '=') {
      *slot = strndup(start, 1);
      ++count;
      ++offset;
      continue;
    }
    const size_t LENGTH = strcspn(start, " \t=");
    *slot = strndup(start, LENGTH);
    ++count;
    offset += LENGTH;
    const char CURRENT = *checked_string_suffix(buffer, offset);
    if (CURRENT != '=' && CURRENT != '\0')
      ++offset;
  }
  if (offset < BUFFER_LENGTH) {
    *text_slot(args, ARGUMENT_CAPACITY, ARGUMENT_CAPACITY - 1) =
        strdup(checked_string_suffix(buffer, offset));
    ++count;
  }
  return (int)count;
}

int silly_parseattributes(char *buffer, char **args, int max) {
  if (max <= 0)
    return 0;
  const size_t ARGUMENT_CAPACITY = (size_t)max;
  memset((void *)args, 0, sizeof(*args) * ARGUMENT_CAPACITY);

  char expanded[LBUF_SIZE];
  size_t output = 0;
  const size_t INPUT_LENGTH = strlen(buffer);
  for (size_t input = 0; input < INPUT_LENGTH; ++input) {
    const char VALUE = *checked_string_suffix(buffer, input);
    const size_t NEEDED = VALUE == '=' ? 3 : 1;
    if (output + NEEDED >= sizeof(expanded))
      return 0;
    if (VALUE == '=') {
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = ' ';
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = '=';
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = ' ';
    } else {
      *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                                  output++) = VALUE;
    }
  }
  *(char *)checked_storage_at(expanded, sizeof(expanded), sizeof(char),
                              output) = '\0';

  char *save_pointer = nullptr;
  char *parsed = strtok_r(expanded, " \t", &save_pointer);
  size_t count = 0;
  while (parsed != nullptr && count < ARGUMENT_CAPACITY) {
    if (count == ARGUMENT_CAPACITY - 1) {
      char remainder[LBUF_SIZE];
      (void)snprintf(remainder, sizeof(remainder), "%s", parsed);
      while ((parsed = strtok_r(nullptr, " \t", &save_pointer)) != nullptr) {
        const size_t USED = strlen(remainder);
        (void)snprintf(checked_storage_at(remainder, sizeof(remainder),
                                          sizeof(char), USED),
                       sizeof(remainder) - USED, " %s", parsed);
      }
      *text_slot(args, ARGUMENT_CAPACITY, count) = strdup(remainder);
    } else {
      *text_slot(args, ARGUMENT_CAPACITY, count) = strdup(parsed);
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
  const size_t ARGUMENT_CAPACITY = (size_t)max;
  memset((void *)args, 0, sizeof(*args) * ARGUMENT_CAPACITY);

  char *storage = strdup(buffer);
  if (storage == nullptr)
    return 0;
  char *remaining = storage;
  while (count < max - 1 && remaining != nullptr && *remaining) {
    char *field = strsep(&remaining, " \t");
    char **slot = (char **)checked_storage_at((void *)args, ARGUMENT_CAPACITY,
                                              sizeof(char *), (size_t)count);
    *slot = strdup(field);
    count++;
  }
  if (remaining != nullptr && *remaining) {
    char **slot = (char **)checked_storage_at(
        (void *)args, ARGUMENT_CAPACITY, sizeof(char *), ARGUMENT_CAPACITY - 1);
    *slot = strdup(remaining);
    count++;
  }
  free(storage);
  return count;
}

int mech_parseattributes(char *buffer, char **args, int maxargs) {
  int count = 0;
  char *parsed = buffer;
  char *token_context = nullptr;
  int num_args = 0;

  if (maxargs <= 0)
    return 0;
  const size_t ARGUMENT_CAPACITY = (size_t)maxargs;
  memset((void *)args, 0, sizeof(*args) * ARGUMENT_CAPACITY);

  while ((count < maxargs) && parsed) {
    parsed = strtok_r(!count ? buffer : nullptr, " \t", &token_context);
    *text_slot(args, ARGUMENT_CAPACITY, (size_t)count) = parsed;
    if (parsed)
      num_args++; /* Actual count of arguments */
    count++;      /* Loop to make sure we don't overrun our */
                  /* buffer */
  }
  return num_args;
}
