/*
 * look.c -- commands which look at things
 */

#include "mux/server/platform.h"

#include "mux/commands/action_messages.h"
#include "mux/commands/command.h"
#include "mux/commands/command_runtime.h"
#include "mux/commands/look.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text.h"
#include "mux/world/match.h"
#include "mux/world/object_set.h"
#include "mux/world/walkdb.h"
#include "mux/world/world_context.h"

#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <math.h>

extern void ufun(char *, char *, int, int, int, DbRef, DbRef);

static void examine_notify_markup(EvaluationContext *evaluation, DbRef player,
                                  const char *label, const char *styled) {
  char *markup = alloc_lbuf("examine_notify_markup");

  if (!styled_text_escape(styled, markup, LBUF_SIZE))
    styled_text_strip(evaluation->world->styled_text_palette, styled, markup,
                      LBUF_SIZE);
  if (label)
    notify_printf(evaluation, player, "%s: %s", label, markup);
  else
    notify(evaluation, player, markup);
  free_lbuf(markup);
}

static void look_exits(EvaluationContext *evaluation, DbRef player, DbRef loc,
                       const char *exit_name) {
  WorldContext *world = evaluation->world;
  DbRef thing;
  char *buff, *e, *s, *buff1, *e1;
  int foundany, key;

  /*
   * make sure location has exits
   */

  if (!is_good_obj(evaluation->world->database, loc) ||
      !has_exits(evaluation->world->database, loc))
    return;

  /*
   * make sure there is at least one visible exit
   */

  foundany = 0;
  key = 0;
  if (is_dark(evaluation->world->database, loc))
    key |= VE_LOC_DARK;
  DOLIST(evaluation->world->database, thing,
         game_object_exits(evaluation->world->database, loc)) {
    if (exit_displayable(world->database, thing, player, key)) {
      foundany = 1;
      break;
    }
  }

  if (!foundany)
    return;
  /*
   * Display the list of exit names
   */

  notify(evaluation, player, exit_name);
  e = buff = alloc_lbuf("look_exits");
  e1 = buff1 = alloc_lbuf("look_exits2");
  if (is_transparent(evaluation->world->database, loc)) {
    DOLIST(evaluation->world->database, thing,
           game_object_exits(evaluation->world->database, loc)) {
      if (exit_displayable(world->database, thing, player, key)) {
        StringCopy(buff, game_object_name(evaluation->world->database, thing));
        for (e = buff; *e && (*e != ';'); e++)
          ;
        *e = '\0';
        notify_printf(
            evaluation, player, "%s leads to %s.", buff,
            game_object_name(
                evaluation->world->database,
                game_object_location(evaluation->world->database, thing)));
      }
    }
  } else {
    DOLIST(evaluation->world->database, thing,
           game_object_exits(evaluation->world->database, loc)) {
      if (exit_displayable(world->database, thing, player, key)) {
        e1 = buff1;
        if (buff != e)
          safe_str("  ", buff, &e);
        for (s = game_object_name(evaluation->world->database, thing);
             *s && (*s != ';'); s++)
          safe_chr(*s, buff1, &e1);
        *e1 = 0;
        safe_str(buff1, buff, &e);
      }
    }
  }

  if (!(is_transparent(evaluation->world->database, loc))) {
    safe_str("\r\n", buff, &e);
    *e = 0;
    notify(evaluation, player, buff);
  }
  free_lbuf(buff);
  free_lbuf(buff1);
}

#define CONTENTS_LOCAL 0
#define CONTENTS_NESTED 1

static void look_contents(EvaluationContext *evaluation, DbRef player,
                          DbRef loc, const char *contents_name, int style) {
  DbRef thing;
  int can_see_loc;
  char *buff;

  /*
   * check to see if he can see the location
   */

  can_see_loc = !is_dark(evaluation->world->database, loc);

  /*
   * check to see if there is anything there
   */

  DOLIST(evaluation->world->database, thing,
         game_object_contents(evaluation->world->database, loc)) {
    if (can_see(evaluation, player, thing, can_see_loc)) {

      /*
       * something exists!  show him everything
       */

      notify(evaluation, player, contents_name);
      DOLIST(evaluation->world->database, thing,
             game_object_contents(evaluation->world->database, loc)) {
        if (can_see(evaluation, player, thing, can_see_loc)) {
          buff = unparse_object(evaluation->world->database, evaluation, player,
                                thing);
          notify(evaluation, player, buff);
          free_lbuf(buff);
        }
      }
      break; /*
              * we're done
              */
    }
  }
}

static bool look_custom_appearance(EvaluationContext *evaluation, DbRef player,
                                   DbRef thing) {
  LuaAppearanceResult result;
  const LuaAppearanceType type =
      is_room(evaluation->world->database, thing) ||
              game_object_location(evaluation->world->database, player) == thing
          ? LUA_APPEARANCE_INTERNAL
          : LUA_APPEARANCE_EXTERNAL;

  lua_appearance_evaluate(
      evaluation->runtime->lua_owner->runtime,
      &(LuaAppearanceInvocation){
          .type = type,
          .descriptor =
              evaluation->command ? evaluation->command->descriptor : nullptr,
          .object = thing,
          .enactor = player,
          .cause = player,
      },
      &result);
  if (!result.defined)
    return false;
  if (*result.rendered)
    notify(evaluation, player, result.rendered);
  notify_event(evaluation,
               evaluation->command ? evaluation->command->descriptor : nullptr,
               player, player, thing, LUA_EVENT_DESCRIBE, nullptr, 0);
  return true;
}

static bool look_simple(EvaluationContext *evaluation, DbRef player,
                        DbRef thing) {
  int pattr;
  char *buff;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(evaluation, player))
    return false;

  if (look_custom_appearance(evaluation, player, thing))
    return true;

  /*
   * Get the name and db-number if we can examine it.
   */

  if (is_examinable(evaluation->world->database, player, thing)) {
    buff =
        unparse_object(evaluation->world->database, evaluation, player, thing);
    notify(evaluation, player, buff);
    free_lbuf(buff);
  }
  pattr = A_DESC;
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_DESCRIBE,
                                .operation = LUA_MESSAGE_OPERATION_DESCRIBE,
                                .object = thing,
                                .enactor = player,
                                .cause = player,
                                .source = NOTHING,
                                .destination = NOTHING},
                    .content_attribute = pattr,
                    .enactor_default = "You see nothing special.",
                    .event = LUA_EVENT_DESCRIBE});
  return false;
}

static void show_a_desc(EvaluationContext *evaluation, DbRef player,
                        DbRef loc) {
  notify_action(evaluation,
                &(ActionMessageInvocation){
                    .message = {.type = LUA_MESSAGE_DESCRIBE,
                                .operation = LUA_MESSAGE_OPERATION_DESCRIBE,
                                .object = loc,
                                .enactor = player,
                                .cause = player,
                                .source = NOTHING,
                                .destination = NOTHING},
                    .content_attribute = A_DESC,
                    .event = LUA_EVENT_DESCRIBE});
}

static void show_desc(EvaluationContext *evaluation, DbRef player, DbRef loc,
                      int use_idesc) {
  char *got;
  long aflags;

  if ((typeof_obj(evaluation->world->database, loc) != OBJECT_TYPE_ROOM) &&
      use_idesc) {
    if (*(got = attribute_get(evaluation->world->database, loc, A_IDESC,
                              &aflags)))
      notify_action(
          evaluation,
          &(ActionMessageInvocation){
              .message = {.type = LUA_MESSAGE_DESCRIBE,
                          .operation = LUA_MESSAGE_OPERATION_INSIDE_DESCRIBE,
                          .object = loc,
                          .enactor = player,
                          .cause = player,
                          .source = NOTHING,
                          .destination = NOTHING},
              .content_attribute = A_IDESC,
              .event = LUA_EVENT_DESCRIBE});
    else
      show_a_desc(evaluation, player, loc);
    free_lbuf(got);
  } else {
    show_a_desc(evaluation, player, loc);
  }
}

void look_in(EvaluationContext *evaluation, DbRef player, DbRef loc, int key) {
  char *buff;
  bool custom;
  LuaLockInvocation lock;
  LuaLockResult result;

  /*
   * Only makes sense for things that can hear
   */

  if (!is_hearer(evaluation, player))
    return;

  if (!is_good_obj(evaluation->world->database, loc))
    return;

  custom = look_custom_appearance(evaluation, player, loc);
  if (!custom) {
    buff = unparse_object(evaluation->world->database, evaluation, player, loc);
    notify(evaluation, player, buff);
    free_lbuf(buff);

    show_desc(evaluation, player, loc,
              loc == game_object_location(evaluation->world->database, player));
  }

  /*
   * tell him the appropriate messages if he has the key
   */

  if (typeof_obj(evaluation->world->database, loc) == OBJECT_TYPE_ROOM) {
    if (lock_test(evaluation, player, player, player, loc, LUA_LOCK_DEFAULT,
                  LUA_LOCK_OPERATION_LOOK, false, &lock, &result))
      notify_action(evaluation,
                    &(ActionMessageInvocation){
                        .message = {.type = LUA_MESSAGE_SUCCESS,
                                    .operation = LUA_MESSAGE_OPERATION_LOOK,
                                    .object = loc,
                                    .enactor = player,
                                    .cause = player,
                                    .source = NOTHING,
                                    .destination = NOTHING},
                        .event = LUA_EVENT_SUCCESS});
    else
      notify_lock_failure(evaluation, &lock, &result, nullptr, nullptr,
                          LUA_EVENT_FAIL);
  }
  if (custom)
    return;
  /*
   * tell him the attributes, contents and exits
   */

  look_contents(evaluation, player, loc, "Contents:", CONTENTS_LOCAL);
  if (key & LK_SHOWEXIT)
    look_exits(evaluation, player, loc, "Obvious exits:");
}

void do_look(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *name = invocation->first;
  DbRef thing, loc;
  int look_key;

  look_key = LK_SHOWATTR | LK_SHOWEXIT;

  loc = game_object_location(evaluation->world->database, player);
  if (!name || !*name) {
    thing = loc;
    if (is_good_obj(evaluation->world->database, thing)) {
      if (key & LOOK_OUTSIDE) {
        if (typeof_obj(evaluation->world->database, thing) ==
            OBJECT_TYPE_ROOM) {
          notify_quiet(evaluation, player, "You can't look outside.");
          return;
        }
        thing = game_object_location(evaluation->world->database, thing);
      }
      look_in(evaluation, player, thing, look_key);
    }
    return;
  }
  /*
   * Look for the target locally
   */

  thing = (key & LOOK_OUTSIDE) ? loc : player;
  init_match(&invocation->context->match, thing, name, OBJECT_TYPE_NOTYPE);
  match_exit(&invocation->context->match);
  match_neighbor(&invocation->context->match);
  match_possession(&invocation->context->match);
  match_here(&invocation->context->match);
  match_me(&invocation->context->match);
  thing = match_result(&invocation->context->match);

  /*
   * Not found locally, check possessive
   */

  if (!is_good_obj(evaluation->world->database, thing)) {
    thing = match_status(evaluation, player,
                         match_possessed(&invocation->context->match, player,
                                         ((key & LOOK_OUTSIDE) ? loc : player),
                                         (char *)name, thing));
  }
  /*
   * If we found something, go handle it
   */

  if (is_good_obj(evaluation->world->database, thing)) {
    switch (typeof_obj(evaluation->world->database, thing)) {
    case OBJECT_TYPE_ROOM:
      look_in(evaluation, player, thing, look_key);
      break;
    case OBJECT_TYPE_THING:
    case OBJECT_TYPE_PLAYER:
      if (!look_simple(evaluation, player, thing)) {
        look_contents(evaluation, player, thing, "Carrying:", CONTENTS_NESTED);
      }
      break;
    case OBJECT_TYPE_EXIT:
      if (!look_simple(evaluation, player, thing) &&
          is_transparent(evaluation->world->database, thing) &&
          (game_object_location(evaluation->world->database, thing) !=
           NOTHING)) {
        look_key &= ~LK_SHOWATTR;
        look_in(evaluation, player,
                game_object_location(evaluation->world->database, thing),
                look_key);
      }
      break;
    default:
      (void)look_simple(evaluation, player, thing);
    }
  }
}

static void debug_examine(EvaluationContext *evaluation, DbRef player,
                          DbRef thing) {
  char *buf;

  notify_printf(evaluation, player, "Number  = %ld", thing);
  if (!is_good_obj(evaluation->world->database, thing))
    return;

  notify_printf(evaluation, player, "Name    = %s",
                game_object_name(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Location= %ld",
                game_object_location(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Contents= %ld",
                game_object_contents(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Exits   = %ld",
                game_object_exits(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Link    = %ld",
                game_object_link(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Next    = %ld",
                game_object_next(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Zone    = %ld",
                game_object_zone(evaluation->world->database, thing));
  buf = flag_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Flags   = %s", buf);
  free_mbuf(buf);
  buf = power_description(evaluation->world->database, player, thing);
  notify_printf(evaluation, player, "Powers  = %s", buf);
  free_mbuf(buf);
  notify_printf(evaluation, player, "Lua state entries: %zu",
                object_state_count(evaluation->world->database, thing));
}

static void examine_state_namespaces(EvaluationContext *evaluation,
                                     DbRef player, DbRef thing) {
  GameDatabase *database = evaluation->world->database;
  const size_t entry_count = object_state_count(database, thing);
  const char *name_space = nullptr;
  size_t namespace_count = 0;

  if (!entry_count)
    return;

  notify_quiet(evaluation, player, "State namespaces:");
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
  examine_state_namespaces(evaluation, player, thing);
  notify_quiet(evaluation, player,
               "Type @state/examine <object>/<namespace> to list the values "
               "in a namespace.");
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
      notify_quiet(evaluation, player, "You must specify an object.");
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
    notify_quiet(evaluation, player, "Invalid state namespace.");
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
    notify_quiet(evaluation, invocation->player, "You must specify an object.");
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
    notify_quiet(evaluation, invocation->player,
                 "Expected <object>/<namespace> <attribute_name>.");
    return false;
  }
  slash = strchr(target, '/');
  if (!slash || slash == target || !slash[1]) {
    notify_quiet(evaluation, invocation->player,
                 "Expected <object>/<namespace> <attribute_name>.");
    return false;
  }
  *slash++ = '\0';
  address->name_space = slash;
  if (!object_state_name_is_valid(address->name_space) ||
      !object_state_name_is_valid(address->key)) {
    notify_quiet(evaluation, invocation->player,
                 "Invalid state namespace or attribute name.");
    return false;
  }
  return state_match_object(invocation, target, &address->object);
}

static bool state_parse_destination(CommandInvocation *invocation, char *text,
                                    char **name_space, char **key) {
  EvaluationContext *evaluation = &invocation->context->evaluation;

  if (!state_split_last_word(text, name_space, key)) {
    notify_quiet(evaluation, invocation->player,
                 "Expected <namespace> <attribute_name>.");
    return false;
  }
  if (!object_state_name_is_valid(*name_space) ||
      !object_state_name_is_valid(*key)) {
    notify_quiet(evaluation, invocation->player,
                 "Invalid state namespace or attribute name.");
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
    notify_quiet(evaluation, invocation->player, "State value cleared.");
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
  notify_quiet(evaluation, invocation->player, "State value set.");
}

static void do_state_wipe(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  GameDatabase *database = evaluation->world->database;
  char *target = invocation->first;
  char *name_space;
  DbRef object;
  size_t removed = 0;

  if (!target || !*target) {
    notify_quiet(evaluation, invocation->player,
                 "Expected <object> or <object>/<namespace>.");
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
      notify_quiet(evaluation, invocation->player, "Invalid state namespace.");
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
    notify_quiet(evaluation, invocation->player,
                 "Unable to start state transaction.");
    object_state_transaction_destroy(&transaction);
    return;
  }
  const ObjectStateValue *value = object_state_transaction_get(
      &transaction, source.object, source.name_space, source.key);
  if (!value) {
    object_state_transaction_finish(&transaction, false);
    object_state_transaction_destroy(&transaction);
    notify_quiet(evaluation, invocation->player,
                 "Source state value not found.");
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

void do_examine(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  const int key = invocation->key;
  char *name = invocation->first;
  DbRef thing, content, exit, loc;
  char *temp, *buf2;
  long aflags;

  /*
   * This command is pointless if the player can't hear.
   */

  if (!is_hearer(evaluation, player))
    return;

  thing = NOTHING;
  if (!name || !*name) {
    if ((thing = game_object_location(evaluation->world->database, player)) ==
        NOTHING)
      return;
  } else {
    /* Look it up */

    init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  /*
   * Check for the /debug switch
   */

  if (key == EXAM_DEBUG) {
    debug_examine(evaluation, player, thing);
    return;
  }

  buf2 = unparse_object(evaluation->world->database, evaluation, player, thing);
  examine_notify_markup(evaluation, player, nullptr, buf2);
  free_lbuf(buf2);
  buf2 = flag_description(evaluation->world->database, player, thing);
  notify(evaluation, player, buf2);
  free_mbuf(buf2);

  temp = alloc_lbuf("do_examine.info");
  temp = attribute_get_string(evaluation->world->database, temp, thing, A_DESC,
                              &aflags);
  if (*temp)
    examine_notify_markup(evaluation, player, "Desc", temp);
  temp = attribute_get_string(evaluation->world->database, temp, thing, A_IDESC,
                              &aflags);
  if (*temp)
    examine_notify_markup(evaluation, player, "Idesc", temp);

  buf2 = unparse_object(evaluation->world->database, evaluation, player,
                        game_object_zone(evaluation->world->database, thing));
  notify_printf(evaluation, player, "Zone: %s", buf2);
  free_lbuf(buf2);
  lua_examine_object(invocation->context->runtime->lua_owner->runtime,
                     evaluation, player, thing);
  buf2 = power_description(evaluation->world->database, player, thing);
  notify(evaluation, player, buf2);
  free_mbuf(buf2);
  if (!(key & EXAM_BRIEF))
    examine_state_namespaces(evaluation, player, thing);
  /*
   * show him interesting stuff
   */

  /*
   * Contents
   */

  if (game_object_contents(evaluation->world->database, thing) != NOTHING) {
    notify(evaluation, player, "Contents:");
    DOLIST(evaluation->world->database, content,
           game_object_contents(evaluation->world->database, thing)) {
      buf2 = unparse_object(evaluation->world->database, evaluation, player,
                            content);
      notify(evaluation, player, buf2);
      free_lbuf(buf2);
    }
  }
  /*
   * Show stuff that depends on the object type
   */

  switch (typeof_obj(evaluation->world->database, thing)) {
  case OBJECT_TYPE_ROOM:

    /*
     * tell him about exits
     */

    if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
      notify(evaluation, player, "Exits:");
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              exit);
        notify(evaluation, player, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify(evaluation, player, "No exits.");
    }

    /*
     * print dropto if present
     */

    if (game_object_location(evaluation->world->database, thing) != NOTHING) {
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_location(evaluation->world->database, thing));
      notify_printf(evaluation, player, "Dropped objects go to: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case OBJECT_TYPE_THING:
  case OBJECT_TYPE_PLAYER:

    /*
     * tell him about exits
     */

    if (game_object_exits(evaluation->world->database, thing) != NOTHING) {
      notify(evaluation, player, "Exits:");
      DOLIST(evaluation->world->database, exit,
             game_object_exits(evaluation->world->database, thing)) {
        buf2 = unparse_object(evaluation->world->database, evaluation, player,
                              exit);
        notify(evaluation, player, buf2);
        free_lbuf(buf2);
      }
    } else {
      notify(evaluation, player, "No exits.");
    }

    /*
     * print home
     */

    loc = game_object_link(evaluation->world->database, thing);
    buf2 = unparse_object(evaluation->world->database, evaluation, player, loc);
    notify_printf(evaluation, player, "Home: %s", buf2);
    free_lbuf(buf2);

    /*
     * print location if player can link to it
     */

    loc = game_object_location(evaluation->world->database, thing);
    if (loc != NOTHING) {
      buf2 =
          unparse_object(evaluation->world->database, evaluation, player, loc);
      notify_printf(evaluation, player, "Location: %s", buf2);
      free_lbuf(buf2);
    }
    break;
  case OBJECT_TYPE_EXIT:
    buf2 =
        unparse_object(evaluation->world->database, evaluation, player,
                       game_object_exits(evaluation->world->database, thing));
    notify_printf(evaluation, player, "Source: %s", buf2);
    free_lbuf(buf2);

    /*
     * print destination
     */

    switch (game_object_location(evaluation->world->database, thing)) {
    case NOTHING:
      break;
    case HOME:
      notify(evaluation, player, "Destination: *HOME*");
      break;
    default:
      buf2 = unparse_object(
          evaluation->world->database, evaluation, player,
          game_object_location(evaluation->world->database, thing));
      notify_printf(evaluation, player, "Destination: %s", buf2);
      free_lbuf(buf2);
      break;
    }
    break;
  default:
    break;
  }
  free_lbuf(temp);
}

void do_inventory(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  DbRef thing;
  char *buff, *s, *e;

  thing = game_object_contents(evaluation->world->database, player);
  if (thing == NOTHING) {
    notify(evaluation, player, "You aren't carrying anything.");
  } else {
    notify(evaluation, player, "You are carrying:");
    DOLIST(evaluation->world->database, thing, thing) {
      buff = unparse_object(evaluation->world->database, evaluation, player,
                            thing);
      notify(evaluation, player, buff);
      free_lbuf(buff);
    }
  }

  thing = game_object_exits(evaluation->world->database, player);
  if (thing != NOTHING) {
    notify(evaluation, player, "Exits:");
    e = buff = alloc_lbuf("look_exits");
    DOLIST(evaluation->world->database, thing, thing) {
      /*
       * chop off first exit alias to display
       */
      for (s = game_object_name(evaluation->world->database, thing);
           *s && (*s != ';'); s++)
        safe_chr(*s, buff, &e);
      safe_str("  ", buff, &e);
    }
    *e = 0;
    notify(evaluation, player, buff);
    free_lbuf(buff);
  }
}

void do_entrances(CommandInvocation *invocation) {
  WorldContext *world = invocation->context->world;
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const DbRef player = invocation->player;
  char *name = invocation->first;
  DbRef thing, i;
  char *exit, *message;
  int control_thing, count;
  long low_bound, high_bound;

  parse_range(world->database, world->configuration, &name, &low_bound,
              &high_bound);
  if (!name || !*name) {
    if (has_location(evaluation->world->database, player))
      thing = game_object_location(evaluation->world->database, player);
    else
      thing = player;
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  } else {
    init_match(&invocation->context->match, player, name, OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    thing = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, thing))
      return;
  }

  message = alloc_lbuf("do_entrances");
  control_thing = is_examinable(evaluation->world->database, player, thing);
  count = 0;
  for (i = low_bound; i <= high_bound; i++) {
    if (control_thing ||
        is_examinable(evaluation->world->database, player, i)) {
      switch (typeof_obj(evaluation->world->database, i)) {
      case OBJECT_TYPE_EXIT:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit =
              unparse_object(evaluation->world->database, evaluation, player,
                             game_object_exits(evaluation->world->database, i));
          notify_printf(evaluation, player, "%s (%s)", exit,
                        game_object_name(evaluation->world->database, i));
          free_lbuf(exit);
          count++;
        }
        break;
      case OBJECT_TYPE_ROOM:
        if (game_object_location(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i);
          notify_printf(evaluation, player, "%s [dropto]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      case OBJECT_TYPE_THING:
      case OBJECT_TYPE_PLAYER:
        if (game_object_link(evaluation->world->database, i) == thing) {
          exit = unparse_object(evaluation->world->database, evaluation, player,
                                i);
          notify_printf(evaluation, player, "%s [home]", exit);
          free_lbuf(exit);
          count++;
        }
        break;
      default:
        break;
      }
    }
  }
  free_lbuf(message);
  notify_printf(evaluation, player, "%d entrance%s found.", count,
                (count == 1) ? "" : "s");
}

/*
 * check the current location for bugs
 */
