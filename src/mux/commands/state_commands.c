/*
 * look.c -- commands which look at things
 */

#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/state_commands.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/access.h"
#include "mux/world/match.h"

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

void state_examine_namespaces(const ObjectStateExamineRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  DbRef player = request->viewer;
  DbRef thing = request->object;
  GameDatabase *database = evaluation->world->database;
  const size_t entry_count = object_state_count(database, thing);
  const char *name_space = nullptr;
  size_t namespace_count = 0;

  if (!entry_count)
    return;

  notify_checked(evaluation, player, player, "State namespaces:", MSG_ME);
  for (size_t index = 0; index < entry_count; index++) {
    ObjectStateEntryResult result =
        object_state_entry(&(ObjectStateEntryRequest){
            .database = database, .object = thing, .index = index});
    if (!result.found)
      continue;
    ObjectStateEntryView entry = result.entry;
    if (name_space && strcmp(name_space, entry.name_space) != 0) {
      notify_printf(evaluation, player, "  %s: %zu value%s", name_space,
                    namespace_count, namespace_count == 1 ? "" : "s");
      namespace_count = 0;
    }
    name_space = entry.name_space;
    namespace_count++;
  }
  if (name_space)
    notify_printf(evaluation, player, "  %s: %zu value%s", name_space,
                  namespace_count, namespace_count == 1 ? "" : "s");
}

static char *examine_state_string(const ObjectStateString *string) {
  char *rendered = alloc_lbuf("examine_state_string");
  char *cursor = rendered;

  safe_chr('"', rendered, &cursor);
  for (size_t index = 0; index < string->length; index++) {
    const unsigned char byte = *(const unsigned char *)checked_storage_at_const(
        string->data, string->length, sizeof(unsigned char), index);
    char escaped[5];

    switch (byte) {
    case '"':
      safe_str("\\\"", rendered, &cursor);
      break;
    case '\\':
      safe_str("\\\\", rendered, &cursor);
      break;
    case '\n':
      safe_str("\\n", rendered, &cursor);
      break;
    case '\r':
      safe_str("\\r", rendered, &cursor);
      break;
    case '\t':
      safe_str("\\t", rendered, &cursor);
      break;
    default:
      if (byte >= 0x20 && byte <= 0x7e) {
        safe_chr((char)byte, rendered, &cursor);
      } else {
        (void)snprintf(escaped, sizeof(escaped), "\\x%02X", byte);
        safe_str(escaped, rendered, &cursor);
      }
      break;
    }
  }
  safe_chr('"', rendered, &cursor);
  *cursor = '\0';
  return rendered;
}

static void examine_state_value(EvaluationContext *evaluation, DbRef player,
                                const ObjectStateEntryView *entry) {
  switch (entry->value->type) {
  case OBJECT_STATE_STRING: {
    char *rendered = examine_state_string(&entry->value->as.string);

    notify_printf(evaluation, player, "  %s (string): %s", entry->key,
                  rendered);
    free_lbuf(rendered);
    break;
  }
  case OBJECT_STATE_BOOLEAN:
    notify_printf(evaluation, player, "  %s (boolean): %s", entry->key,
                  entry->value->as.boolean ? "true" : "false");
    break;
  case OBJECT_STATE_INTEGER:
    notify_printf(evaluation, player, "  %s (integer): %" PRId64, entry->key,
                  entry->value->as.integer);
    break;
  case OBJECT_STATE_NUMBER:
    notify_printf(evaluation, player, "  %s (number): %.17g", entry->key,
                  entry->value->as.number);
    break;
  }
}

typedef struct StateNamespaceExamineRequest {
  ObjectStateExamineRequest object;
  const char *name_space;
} StateNamespaceExamineRequest;

static void
examine_state_namespace(const StateNamespaceExamineRequest *request) {
  EvaluationContext *evaluation = request->object.evaluation;
  DbRef player = request->object.viewer;
  DbRef thing = request->object.object;
  const char *name_space = request->name_space;
  GameDatabase *database = evaluation->world->database;
  const size_t entry_count = object_state_count(database, thing);
  bool found = false;

  for (size_t index = 0; index < entry_count; index++) {
    ObjectStateEntryResult result =
        object_state_entry(&(ObjectStateEntryRequest){
            .database = database, .object = thing, .index = index});
    ObjectStateEntryView entry = result.entry;
    if (!result.found || strcmp(entry.name_space, name_space) != 0)
      continue;
    if (!found)
      notify_printf(evaluation, player, "State namespace %s:", name_space);
    examine_state_value(evaluation, player, &entry);
    found = true;
  }
  if (!found)
    notify_printf(evaluation, player, "No state namespace named %s.",
                  name_space);
}

static void examine_state_summary(EvaluationContext *evaluation, DbRef player,
                                  DbRef thing) {
  state_examine_namespaces(&(ObjectStateExamineRequest){
      .evaluation = evaluation, .viewer = player, .object = thing});
  notify_checked(evaluation, player, player,
                 "Type @state/examine <object>/<namespace> to list the values "
                 "in a namespace.",
                 MSG_ME);
}

static void do_state_examine(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  char *name = invocation->first;
  char *name_space = nullptr;
  DbRef thing;

  if (!is_hearer(evaluation, player))
    return;

  if (!name || !*name) {
    thing = game_object_location(evaluation->world->database, player);
  } else {
    const size_t name_length = strlen(name);
    size_t slash_offset = 0;

    while (slash_offset < name_length &&
           *(const char *)checked_storage_at_const(
               name, name_length + 1, sizeof(char), slash_offset) != '/')
      slash_offset++;
    if (slash_offset < name_length) {
      *(char *)checked_storage_at(name, name_length + 1, sizeof(char),
                                  slash_offset) = '\0';
      name_space = checked_storage_at(name, name_length + 1, sizeof(char),
                                      slash_offset + 1);
    }
    if (!*name) {
      notify_checked(evaluation, player, player, "You must specify an object.",
                     MSG_ME);
      return;
    }
    init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
  }
  if (!is_good_obj(evaluation->world->database, thing))
    return;
  if (!name_space) {
    examine_state_summary(evaluation, player, thing);
  } else if (!object_state_name_is_valid(name_space)) {
    notify_checked(evaluation, player, player, "Invalid state namespace.",
                   MSG_ME);
  } else {
    examine_state_namespace(&(StateNamespaceExamineRequest){
        .object = {.evaluation = evaluation, .viewer = player, .object = thing},
        .name_space = name_space});
  }
}

typedef struct StateAddress StateAddress;
struct StateAddress {
  DbRef object;
  char *name_space;
  char *key;
};

typedef struct StateWordSplitResult {
  bool found;
  char *prefix;
  char *word;
} StateWordSplitResult;

static StateWordSplitResult state_split_last_word(char *text) {
  if (!text || !*text)
    return (StateWordSplitResult){0};
  const size_t length = strlen(text);
  size_t end = length;
  size_t start;
  size_t separator;

  while (end > 0 &&
         (isspace)((unsigned char)*(const char *)checked_storage_at_const(
             text, length + 1, sizeof(char), end - 1)))
    end--;
  *(char *)checked_storage_at(text, length + 1, sizeof(char), end) = '\0';
  start = end;
  while (start > 0 &&
         !(isspace)((unsigned char)*(const char *)checked_storage_at_const(
             text, length + 1, sizeof(char), start - 1)))
    start--;
  if (start == 0)
    return (StateWordSplitResult){0};
  separator = start;
  while (separator > 0 &&
         (isspace)((unsigned char)*(const char *)checked_storage_at_const(
             text, length + 1, sizeof(char), separator - 1)))
    separator--;
  *(char *)checked_storage_at(text, length + 1, sizeof(char), separator) = '\0';
  char *word_start = checked_storage_at(text, length + 1, sizeof(char), start);

  if (!*text || !*word_start)
    return (StateWordSplitResult){0};
  return (StateWordSplitResult){
      .found = true, .prefix = text, .word = word_start};
}

static bool state_match_object(CommandInvocation *invocation, char *name,
                               DbRef *object) {
  EvaluationContext *evaluation = &invocation->context->evaluation;

  if (!name || !*name) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "You must specify an object.", MSG_ME);
    return false;
  }
  init_match(&invocation->context->match, invocation->player, name,
             OBJECT_TYPE_NOTYPE);
  match_everything(&invocation->context->match, 0);
  *object = noisy_match_result(&invocation->context->match);
  return is_good_obj(evaluation->world->database, *object);
}

static bool state_parse_address(CommandInvocation *invocation, char *text,
                                StateAddress *address) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  char *target;
  char *slash;

  StateWordSplitResult split = state_split_last_word(text);
  if (!split.found) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Expected <object>/<namespace> <attribute_name>.", MSG_ME);
    return false;
  }
  target = split.prefix;
  address->key = split.word;
  const size_t target_length = strlen(target);
  size_t slash_offset = 0;

  while (slash_offset < target_length &&
         *(const char *)checked_storage_at_const(
             target, target_length + 1, sizeof(char), slash_offset) != '/')
    slash_offset++;
  if (slash_offset == 0 || slash_offset + 1 >= target_length) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Expected <object>/<namespace> <attribute_name>.", MSG_ME);
    return false;
  }
  slash =
      checked_storage_at(target, target_length + 1, sizeof(char), slash_offset);
  *slash = '\0';
  slash = checked_storage_at(target, target_length + 1, sizeof(char),
                             slash_offset + 1);
  address->name_space = slash;
  if (!object_state_name_is_valid(address->name_space) ||
      !object_state_name_is_valid(address->key)) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Invalid state namespace or attribute name.", MSG_ME);
    return false;
  }
  return state_match_object(invocation, target, &address->object);
}

static bool state_parse_destination(CommandInvocation *invocation, char *text,
                                    char **name_space, char **key) {
  EvaluationContext *evaluation = &invocation->context->evaluation;

  StateWordSplitResult split = state_split_last_word(text);
  if (!split.found) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Expected <namespace> <attribute_name>.", MSG_ME);
    return false;
  }
  *name_space = split.prefix;
  *key = split.word;
  if (!object_state_name_is_valid(*name_space) ||
      !object_state_name_is_valid(*key)) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Invalid state namespace or attribute name.", MSG_ME);
    return false;
  }
  return true;
}

static int state_hex_digit(unsigned char byte) {
  if (byte >= '0' && byte <= '9')
    return byte - '0';
  if (byte >= 'a' && byte <= 'f')
    return byte - 'a' + 10;
  if (byte >= 'A' && byte <= 'F')
    return byte - 'A' + 10;
  return -1;
}

static bool state_parse_quoted_string(const char *text, ObjectStateValue *value,
                                      char **owned, char *error,
                                      size_t error_size) {
  const size_t length = strlen(text);
  char *decoded;
  size_t output = 0;

  if (length < 2 || *(const char *)checked_storage_at_const(
                        text, length + 1, sizeof(char), length - 1) != '"') {
    (void)snprintf(error, error_size, "unterminated quoted string");
    return false;
  }
  decoded = malloc(length);
  if (!decoded) {
    (void)snprintf(error, error_size, "out of memory");
    return false;
  }
  for (size_t index = 1; index < length - 1; index++) {
    unsigned char byte = (unsigned char)*(const char *)checked_storage_at_const(
        text, length + 1, sizeof(char), index);

    if (byte != '\\') {
      *(char *)checked_storage_at(decoded, length, sizeof(char), output++) =
          (char)byte;
      continue;
    }
    if (++index >= length - 1) {
      free(decoded);
      (void)snprintf(error, error_size, "incomplete string escape");
      return false;
    }
    byte = (unsigned char)*(const char *)checked_storage_at_const(
        text, length + 1, sizeof(char), index);
    switch (byte) {
    case '"':
    case '\\':
      *(char *)checked_storage_at(decoded, length, sizeof(char), output++) =
          (char)byte;
      break;
    case 'n':
      *(char *)checked_storage_at(decoded, length, sizeof(char), output++) =
          '\n';
      break;
    case 'r':
      *(char *)checked_storage_at(decoded, length, sizeof(char), output++) =
          '\r';
      break;
    case 't':
      *(char *)checked_storage_at(decoded, length, sizeof(char), output++) =
          '\t';
      break;
    case 'x': {
      int high;
      int low;

      bool invalid_escape = index + 2 >= length - 1;
      if (!invalid_escape) {
        high = state_hex_digit(
            (unsigned char)*(const char *)checked_storage_at_const(
                text, length + 1, sizeof(char), index + 1));
        invalid_escape = high < 0;
      }
      if (!invalid_escape) {
        low = state_hex_digit(
            (unsigned char)*(const char *)checked_storage_at_const(
                text, length + 1, sizeof(char), index + 2));
        invalid_escape = low < 0;
      }
      if (invalid_escape) {
        free(decoded);
        (void)snprintf(error, error_size, "invalid hexadecimal string escape");
        return false;
      }
      *(char *)checked_storage_at(decoded, length, sizeof(char), output++) =
          (char)((high << 4) | low);
      index += 2;
      break;
    }
    default:
      free(decoded);
      (void)snprintf(error, error_size, "unknown string escape");
      return false;
    }
  }
  *(char *)checked_storage_at(decoded, length, sizeof(char), output) = '\0';
  *owned = decoded;
  *value = (ObjectStateValue){
      .type = OBJECT_STATE_STRING,
      .as.string = {.data = decoded, .length = output},
  };
  return true;
}

static bool state_parse_value(const char *text, ObjectStateValue *value,
                              char **owned, char *error, size_t error_size) {
  char *end;

  *owned = nullptr;
  if (*text == '"')
    return state_parse_quoted_string(text, value, owned, error, error_size);
  if (!strcmp(text, "true") || !strcmp(text, "false")) {
    *value = (ObjectStateValue){
        .type = OBJECT_STATE_BOOLEAN,
        .as.boolean = !strcmp(text, "true"),
    };
    return true;
  }

  errno = 0;
  intmax_t integer = strtoimax(text, &end, 10);
  if (!errno && *text && !*end && integer >= INT64_MIN &&
      integer <= INT64_MAX) {
    *value = (ObjectStateValue){
        .type = OBJECT_STATE_INTEGER,
        .as.integer = (int64_t)integer,
    };
    return true;
  }

  errno = 0;
  double number = strtod(text, &end);
  if (!errno && *text && !*end && isfinite(number)) {
    *value = (ObjectStateValue){
        .type = OBJECT_STATE_NUMBER,
        .as.number = number,
    };
    return true;
  }

  *value = (ObjectStateValue){
      .type = OBJECT_STATE_STRING,
      .as.string = {.data = text, .length = strlen(text)},
  };
  return true;
}

static void do_state_set(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  GameDatabase *database = evaluation->world->database;
  StateAddress address;
  const char *text = invocation->second ? invocation->second : "";

  if (!state_parse_address(invocation, invocation->first, &address))
    return;
  if (!*text) {
    object_state_delete(database, address.object, address.name_space,
                        address.key);
    notify_checked(evaluation, invocation->player, invocation->player,
                   "State value cleared.", MSG_ME);
    return;
  }

  ObjectStateValue value;
  char *owned;
  char error[LBUF_SIZE];
  if (!state_parse_value(text, &value, &owned, error, sizeof(error))) {
    notify_printf(evaluation, invocation->player, "Unable to set state: %s.",
                  error);
    return;
  }
  bool set = object_state_set(database, address.object, address.name_space,
                              address.key, &value, error, sizeof(error));
  free(owned);
  if (!set) {
    notify_printf(evaluation, invocation->player, "Unable to set state: %s.",
                  error);
    return;
  }
  notify_checked(evaluation, invocation->player, invocation->player,
                 "State value set.", MSG_ME);
}

static void do_state_wipe(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  GameDatabase *database = evaluation->world->database;
  char *target = invocation->first;
  char *name_space;
  DbRef object;
  size_t removed = 0;

  if (!target || !*target) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Expected <object> or <object>/<namespace>.", MSG_ME);
    return;
  }
  const size_t target_length = strlen(target);
  size_t slash_offset = 0;

  while (slash_offset < target_length &&
         *(const char *)checked_storage_at_const(
             target, target_length + 1, sizeof(char), slash_offset) != '/')
    slash_offset++;
  name_space = nullptr;
  if (slash_offset < target_length) {
    *(char *)checked_storage_at(target, target_length + 1, sizeof(char),
                                slash_offset) = '\0';
    name_space = checked_storage_at(target, target_length + 1, sizeof(char),
                                    slash_offset + 1);
  }
  if (!state_match_object(invocation, target, &object))
    return;
  if (!name_space) {
    removed = object_state_count(database, object);
    object_state_clear(database, object);
  } else {
    if (!object_state_name_is_valid(name_space)) {
      notify_checked(evaluation, invocation->player, invocation->player,
                     "Invalid state namespace.", MSG_ME);
      return;
    }
    for (size_t index = 0; index < object_state_count(database, object);) {
      ObjectStateEntryResult result =
          object_state_entry(&(ObjectStateEntryRequest){
              .database = database, .object = object, .index = index});
      if (!result.found)
        break;
      ObjectStateEntryView entry = result.entry;
      if (strcmp(entry.name_space, name_space) != 0) {
        index++;
        continue;
      }
      if (!object_state_delete(database, object, name_space, entry.key))
        break;
      removed++;
    }
  }
  notify_printf(evaluation, invocation->player, "%zu state value%s wiped.",
                removed, removed == 1 ? "" : "s");
}

static void do_state_copy_or_move(CommandInvocation *invocation, bool move) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  GameDatabase *database = evaluation->world->database;
  StateAddress source;
  char *destination_namespace;
  char *destination_key;
  char error[LBUF_SIZE] = {0};

  if (!state_parse_address(invocation, invocation->first, &source) ||
      !state_parse_destination(invocation, invocation->second,
                               &destination_namespace, &destination_key))
    return;

  ObjectStateTransaction transaction;
  object_state_transaction_initialize(&transaction);
  if (!object_state_transaction_begin(&transaction, database)) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Unable to start state transaction.", MSG_ME);
    object_state_transaction_destroy(&transaction);
    return;
  }
  const ObjectStateValue *value = object_state_transaction_get(
      &transaction, source.object, source.name_space, source.key);
  if (!value) {
    object_state_transaction_finish(&transaction, false);
    object_state_transaction_destroy(&transaction);
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Source state value not found.", MSG_ME);
    return;
  }
  bool changed = object_state_transaction_set(
      &transaction, source.object, destination_namespace, destination_key,
      value, error, sizeof(error));
  if (changed && move &&
      (strcmp(source.name_space, destination_namespace) != 0 ||
       strcmp(source.key, destination_key) != 0))
    changed = object_state_transaction_delete(&transaction, source.object,
                                              source.name_space, source.key);
  object_state_transaction_finish(&transaction, changed);
  object_state_transaction_destroy(&transaction);
  if (!changed) {
    notify_printf(evaluation, invocation->player,
                  "Unable to %s state value: %s.", move ? "move" : "copy",
                  *error ? error : "transaction failed");
    return;
  }
  notify_printf(evaluation, invocation->player, "State value %s.",
                move ? "moved" : "copied");
}

void do_state(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;

  switch (invocation->key) {
  case 0:
    raw_notify(evaluation, invocation->player, "@state command switches:");
    raw_notify(evaluation, invocation->player,
               "  /examine  Inspect persistent object state.");
    raw_notify(evaluation, invocation->player,
               "  /set      Set or clear a state value.");
    raw_notify(evaluation, invocation->player,
               "  /wipe     Clear object state or one namespace.");
    raw_notify(evaluation, invocation->player,
               "  /copy     Copy a state value on an object.");
    raw_notify(evaluation, invocation->player,
               "  /move     Move a state value on an object.");
    return;
  case STATE_COMMAND_EXAMINE:
    do_state_examine(invocation);
    return;
  case STATE_COMMAND_SET:
    do_state_set(invocation);
    return;
  case STATE_COMMAND_WIPE:
    do_state_wipe(invocation);
    return;
  case STATE_COMMAND_COPY:
    do_state_copy_or_move(invocation, false);
    return;
  case STATE_COMMAND_MOVE:
    do_state_copy_or_move(invocation, true);
    return;
  default:
    raw_notify(evaluation, invocation->player,
               "Invalid @state switch combination.");
    return;
  }
}
