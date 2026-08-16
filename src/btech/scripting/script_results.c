/* Implements typed results for BattleTech scripting functions. */

#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "script_functions_api.h"

static bool btech_script_list_separator(char character) {
  return (character == ' ' || character == '|') != 0;
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
  BtechScriptList list = {};
  list.count = btech_script_list_item_count(output);
  if (list.count == 0)
    return list;
  list.items =
      checked_storage_try_allocate_array(list.count, sizeof(*list.items));
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

BtechScriptResult btech_script_error(BtechScriptCall *call,
                                     const char *message) {
  assert(call != nullptr);
  assert(call->output.buffer != nullptr);
  assert(call->output.capacity > 0);
  assert(message != nullptr);
  (void)string_copy_bounded(call->output.buffer, call->output.capacity,
                            message);
  const size_t LENGTH = strnlen(call->output.buffer, call->output.capacity);
  call->output.cursor = checked_storage_at(
      call->output.buffer, call->output.capacity, sizeof(char), LENGTH);
  return btech_script_error_output(call);
}

BtechScriptResult btech_script_error_output(BtechScriptCall *call) {
  assert(call != nullptr);
  assert(call->output.buffer != nullptr);
  assert(call->output.cursor != nullptr);
  assert(call->output.capacity > 0);
  *call->output.cursor = '\0';
  return (BtechScriptResult){.status = BTECH_SCRIPT_ERROR,
                             .kind = BTECH_SCRIPT_TEXT,
                             .value.text = call->output.buffer};
}

BtechScriptResult btech_script_result_finish(BtechScriptCall *call,
                                             BtechScriptValueKind kind) {
  *call->output.cursor = '\0';
  BtechScriptResult result = {.status = BTECH_SCRIPT_OK, .kind = kind};

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
  *result = (BtechScriptResult){};
}
