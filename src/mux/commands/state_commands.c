/*
 * look.c -- commands which look at things
 */

#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <math.h>
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
#include "mux/world/access.h"
#include "mux/world/match.h"

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

void state_examine_namespaces(EvaluationContext *evaluation, DbRef player,
                              DbRef thing) {
  GameDatabase *database = evaluation->world->database;
  const size_t entry_count = object_state_count(database, thing);
  const char *name_space = nullptr;
  size_t namespace_count = 0;

  if (!entry_count)
    return;

  notify_checked(evaluation, player, player, "State namespaces:", MSG_ME);
  for (size_t index = 0; index < entry_count; index++) {
    ObjectStateEntryView entry;

    if (!object_state_entry(database, thing, index, &entry))
      continue;
    if (name_space && strcmp(name_space, entry.name_space)) {
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
    const unsigned char byte = (unsigned char)string->data[index];
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
        snprintf(escaped, sizeof(escaped), "\\x%02X", byte);
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

static void examine_state_namespace(EvaluationContext *evaluation, DbRef player,
                                    DbRef thing, const char *name_space) {
  GameDatabase *database = evaluation->world->database;
  const size_t entry_count = object_state_count(database, thing);
  bool found = false;

  for (size_t index = 0; index < entry_count; index++) {
    ObjectStateEntryView entry;

    if (!object_state_entry(database, thing, index, &entry) ||
        strcmp(entry.name_space, name_space))
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
  state_examine_namespaces(evaluation, player, thing);
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
    name_space = strchr(name, '/');
    if (name_space)
      *name_space++ = '\0';
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
    examine_state_namespace(evaluation, player, thing, name_space);
  }
}

typedef struct StateAddress StateAddress;
struct StateAddress {
  DbRef object;
  char *name_space;
  char *key;
};

static bool state_split_last_word(char *text, char **prefix, char **word) {
  char *end;
  char *start;
  char *separator;

  if (!text || !*text)
    return false;
  end = text + strlen(text);
  while (end > text && isspace((unsigned char)end[-1]))
    end--;
  *end = '\0';
  start = end;
  while (start > text && !isspace((unsigned char)start[-1]))
    start--;
  if (start == text)
    return false;
  separator = start;
  while (separator > text && isspace((unsigned char)separator[-1]))
    separator--;
  *separator = '\0';
  if (!*text || !*start)
    return false;
  *prefix = text;
  *word = start;
  return true;
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

  if (!state_split_last_word(text, &target, &address->key)) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Expected <object>/<namespace> <attribute_name>.", MSG_ME);
    return false;
  }
  slash = strchr(target, '/');
  if (!slash || slash == target || !slash[1]) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Expected <object>/<namespace> <attribute_name>.", MSG_ME);
    return false;
  }
  *slash++ = '\0';
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

  if (!state_split_last_word(text, name_space, key)) {
    notify_checked(evaluation, invocation->player, invocation->player,
                   "Expected <namespace> <attribute_name>.", MSG_ME);
    return false;
  }
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

  if (length < 2 || text[length - 1] != '"') {
    snprintf(error, error_size, "unterminated quoted string");
    return false;
  }
  decoded = malloc(length);
  if (!decoded) {
    snprintf(error, error_size, "out of memory");
    return false;
  }
  for (size_t index = 1; index < length - 1; index++) {
    unsigned char byte = (unsigned char)text[index];

    if (byte != '\\') {
      decoded[output++] = (char)byte;
      continue;
    }
    if (++index >= length - 1) {
      free(decoded);
      snprintf(error, error_size, "incomplete string escape");
      return false;
    }
    byte = (unsigned char)text[index];
    switch (byte) {
    case '"':
    case '\\':
      decoded[output++] = (char)byte;
      break;
    case 'n':
      decoded[output++] = '\n';
      break;
    case 'r':
      decoded[output++] = '\r';
      break;
    case 't':
      decoded[output++] = '\t';
      break;
    case 'x': {
      int high;
      int low;

      if (index + 2 >= length - 1 ||
          (high = state_hex_digit((unsigned char)text[index + 1])) < 0 ||
          (low = state_hex_digit((unsigned char)text[index + 2])) < 0) {
        free(decoded);
        snprintf(error, error_size, "invalid hexadecimal string escape");
        return false;
      }
      decoded[output++] = (char)((high << 4) | low);
      index += 2;
      break;
    }
    default:
      free(decoded);
      snprintf(error, error_size, "unknown string escape");
      return false;
    }
  }
  decoded[output] = '\0';
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
  name_space = strchr(target, '/');
  if (name_space)
    *name_space++ = '\0';
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
      ObjectStateEntryView entry;

      if (!object_state_entry(database, object, index, &entry))
        break;
      if (strcmp(entry.name_space, name_space)) {
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
      (strcmp(source.name_space, destination_namespace) ||
       strcmp(source.key, destination_key)))
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
