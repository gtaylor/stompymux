/* flags.c - object flag manipulation routines */

#include <ctype.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h" // IWYU pragma: keep
#include "btech/special_objects.h"
#include "mux/commands/command.h"
#include "mux/commands/command_keys.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h" // IWYU pragma: keep
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/lbuf_text.h"
#include "mux/support/stringutil.h"

static bool *object_flag_value_at(ObjectFlagSet *flags, ObjectFlag flag) {
  return checked_storage_at(flags->values, OBJECT_FLAG_COUNT,
                            sizeof(*flags->values), (size_t)flag);
}

static const bool *object_flag_value_at_const(const ObjectFlagSet *flags,
                                              ObjectFlag flag) {
  return checked_storage_at_const(flags->values, OBJECT_FLAG_COUNT,
                                  sizeof(*flags->values), (size_t)flag);
}

bool game_object_has_flag(const ObjectFlagRequest *request) {
  GameDatabase *database = request->database;
  DbRef object = request->object;
  ObjectFlag flag = request->flag;
  const GameObject *game_object = game_database_object(database, object);

  switch (flag) {
  case OBJECT_FLAG_ANSI:
    return game_object->has_ansi_flag;
  case OBJECT_FLAG_AUDIBLE:
    return game_object->has_audible_flag;
  case OBJECT_FLAG_AUDITORIUM:
    return game_object->has_auditorium_flag;
  case OBJECT_FLAG_BLIND:
    return game_object->has_blind_flag;
  case OBJECT_FLAG_CONNECTED:
    return game_object->has_connected_flag;
  case OBJECT_FLAG_DARK:
    return game_object->has_dark_flag;
  case OBJECT_FLAG_FLOATING:
    return game_object->has_floating_flag;
  case OBJECT_FLAG_GAGGED:
    return game_object->has_gagged_flag;
  case OBJECT_FLAG_GOING:
    return game_object->has_going_flag;
  case OBJECT_FLAG_HALTED:
    return game_object->has_halted_flag;
  case OBJECT_FLAG_IN_CHARACTER:
    return game_object->has_in_character_flag;
  case OBJECT_FLAG_LIGHT:
    return game_object->has_light_flag;
  case OBJECT_FLAG_MONITOR:
    return game_object->has_monitor_flag;
  case OBJECT_FLAG_NO_COMMAND:
    return game_object->has_no_command_flag;
  case OBJECT_FLAG_SAFE:
    return game_object->has_safe_flag;
  case OBJECT_FLAG_SUSPECT:
    return game_object->has_suspect_flag;
  case OBJECT_FLAG_TRANSPARENT:
    return game_object->has_transparent_flag;
  case OBJECT_FLAG_WIZARD:
    return game_object->has_wizard_flag;
  case OBJECT_FLAG_XCODE:
    return game_object->has_xcode_flag;
  case OBJECT_FLAG_ZOMBIE:
    return game_object->has_zombie_flag;
  case OBJECT_FLAG_NONE:
  case OBJECT_FLAG_COUNT:
    return false;
  }
  return false;
}

void game_object_set_flag(const ObjectFlagChangeRequest *request) {
  GameDatabase *database = request->database;
  DbRef object = request->object;
  ObjectFlag flag = request->flag;
  bool value = request->value;
  GameObject *game_object = game_database_object(database, object);

  switch (flag) {
  case OBJECT_FLAG_ANSI:
    game_object->has_ansi_flag = value;
    break;
  case OBJECT_FLAG_AUDIBLE:
    game_object->has_audible_flag = value;
    break;
  case OBJECT_FLAG_AUDITORIUM:
    game_object->has_auditorium_flag = value;
    break;
  case OBJECT_FLAG_BLIND:
    game_object->has_blind_flag = value;
    break;
  case OBJECT_FLAG_CONNECTED:
    game_object->has_connected_flag = value;
    break;
  case OBJECT_FLAG_DARK:
    game_object->has_dark_flag = value;
    break;
  case OBJECT_FLAG_FLOATING:
    game_object->has_floating_flag = value;
    break;
  case OBJECT_FLAG_GAGGED:
    game_object->has_gagged_flag = value;
    break;
  case OBJECT_FLAG_GOING:
    game_object->has_going_flag = value;
    break;
  case OBJECT_FLAG_HALTED:
    game_object->has_halted_flag = value;
    break;
  case OBJECT_FLAG_IN_CHARACTER:
    game_object->has_in_character_flag = value;
    break;
  case OBJECT_FLAG_LIGHT:
    game_object->has_light_flag = value;
    break;
  case OBJECT_FLAG_MONITOR:
    game_object->has_monitor_flag = value;
    break;
  case OBJECT_FLAG_NO_COMMAND:
    game_object->has_no_command_flag = value;
    break;
  case OBJECT_FLAG_SAFE:
    game_object->has_safe_flag = value;
    break;
  case OBJECT_FLAG_SUSPECT:
    game_object->has_suspect_flag = value;
    break;
  case OBJECT_FLAG_TRANSPARENT:
    game_object->has_transparent_flag = value;
    break;
  case OBJECT_FLAG_WIZARD:
    game_object->has_wizard_flag = value;
    break;
  case OBJECT_FLAG_XCODE:
    game_object->has_xcode_flag = value;
    break;
  case OBJECT_FLAG_ZOMBIE:
    game_object->has_zombie_flag = value;
    break;
  case OBJECT_FLAG_NONE:
  case OBJECT_FLAG_COUNT:
    break;
  }
}

void game_object_clear_flags(GameDatabase *database, DbRef object) {
  for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++)
    game_object_set_flag(&(ObjectFlagChangeRequest){
        .database = database, .object = object, .flag = flag});
}

void game_object_flags_copy(GameDatabase *database, DbRef object,
                            ObjectFlagSet *flags) {
  for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++)
    *object_flag_value_at(flags, flag) =
        game_object_has_flag(&(ObjectFlagRequest){
            .database = database, .object = object, .flag = flag});
}

bool object_flag_set_has(const ObjectFlagSet *flags, ObjectFlag flag) {
  return flag > OBJECT_FLAG_NONE && flag < OBJECT_FLAG_COUNT &&
         *object_flag_value_at_const(flags, flag);
}

void object_flag_set_set(ObjectFlagSet *flags, ObjectFlag flag, bool value) {
  if (flag > OBJECT_FLAG_NONE && flag < OBJECT_FLAG_COUNT)
    *object_flag_value_at(flags, flag) = value;
}

bool is_good_obj(GameDatabase *database, DbRef x) {
  return x >= 0 && x < database->top &&
         typeof_obj(database, x) != OBJECT_TYPE_INVALID &&
         typeof_obj(database, x) != OBJECT_TYPE_NOTYPE;
}

bool is_safe(GameDatabase *database, DbRef object) {
  return is_player(database, object) ||
         game_object_has_flag(&(ObjectFlagRequest){
             .database = database, .object = object, .flag = OBJECT_FLAG_SAFE});
}

bool can_link_exit(GameDatabase *database, DbRef player, DbRef target) {
  return is_exit(database, target) &&
         (game_object_location(database, target) == NOTHING ||
          is_controls(database, player, target));
}

bool is_linkable(GameDatabase *database, DbRef player, DbRef target) {
  return is_good_obj(database, target) && has_contents(database, target) &&
         is_controls(database, player, target);
}

void mark(GameDatabase *database, DbRef x) {
  const unsigned char MASK = (unsigned char)(1U << (x & 7));
  char *byte = checked_storage_at(database->markbits->chunk,
                                  sizeof(database->markbits->chunk),
                                  sizeof(char), (size_t)(x >> 3));
  *byte = (char)((unsigned char)*byte | MASK);
}
void unmark(GameDatabase *database, DbRef x) {
  const unsigned char MASK = (unsigned char)(1U << (x & 7));
  char *byte = checked_storage_at(database->markbits->chunk,
                                  sizeof(database->markbits->chunk),
                                  sizeof(char), (size_t)(x >> 3));
  *byte = (char)((unsigned char)*byte & (unsigned char)~MASK);
}
bool is_marked(GameDatabase *database, DbRef x) {
  const char *byte = checked_storage_at_const(database->markbits->chunk,
                                              sizeof(database->markbits->chunk),
                                              sizeof(char), (size_t)(x >> 3));
  return ((unsigned char)*byte & (unsigned char)(1U << (x & 7))) != 0;
}
void unmark_all(GameDatabase *database) {
  for (DbRef index = 0; index < ((database->top + 7) >> 3); index++)
    *(char *)checked_storage_at(database->markbits->chunk,
                                sizeof(database->markbits->chunk), sizeof(char),
                                (size_t)index) = 0;
}

static bool flag_any(const FlagChangeRequest *request) {
  game_object_set_flag(&(ObjectFlagChangeRequest){
      .database = request->evaluation->world->database,
      .object = request->target,
      .flag = request->flag,
      .value = !request->clear});
  return true;
}
static bool flag_god(const FlagChangeRequest *request) {
  return is_god(request->evaluation->world->database, request->player) &&
         flag_any(request);
}
static bool flag_wizard(const FlagChangeRequest *request) {
  return (is_wizard(request->evaluation->world->database, request->player) ||
          is_god(request->evaluation->world->database, request->player)) &&
         flag_any(request);
}
static bool flag_wizard_bit(const FlagChangeRequest *request) {
  if (!is_god(request->evaluation->world->database, request->player))
    return false;
  if (is_god(request->evaluation->world->database, request->target) &&
      request->clear) {
    notify_checked(request->evaluation, request->player, request->player,
                   "You cannot make yourself mortal.", MSG_ME_ALL | MSG_F_DOWN);
    return false;
  }
  return flag_any(request);
}
static bool flag_going(const FlagChangeRequest *request) {
  if (is_going(request->evaluation->world->database, request->target) &&
      request->clear &&
      !is_player(request->evaluation->world->database, request->target))
    return flag_any(request);
  return is_god(request->evaluation->world->database, request->player) &&
         flag_any(request);
}

static bool flag_xcode(const FlagChangeRequest *request) {
  bool previously_enabled =
      is_xcode(request->evaluation->world->database, request->target);
  bool changed;

  if (!flag_wizard(request))
    return false;
  changed = previously_enabled !=
            is_xcode(request->evaluation->world->database, request->target);
  if (changed)
    btech_special_object_flag_changed(
        request->evaluation->btech, request->player, request->target,
        previously_enabled,
        is_xcode(request->evaluation->world->database, request->target));
  return true;
}

static const FlagEntry FLAG_ENTRIES[] = {
    {"ANSI", OBJECT_FLAG_ANSI, 'X', flag_wizard},
    {"AUDIBLE", OBJECT_FLAG_AUDIBLE, 'a', flag_wizard},
    {"AUDITORIUM", OBJECT_FLAG_AUDITORIUM, 'b', flag_wizard},
    {"BLIND", OBJECT_FLAG_BLIND, '(', flag_wizard},
    {"CONNECTED", OBJECT_FLAG_CONNECTED, 'c', flag_god},
    {"DARK", OBJECT_FLAG_DARK, 'D', flag_wizard},
    {"FLOATING", OBJECT_FLAG_FLOATING, 'F', flag_wizard},
    {"GAGGED", OBJECT_FLAG_GAGGED, 'j', flag_wizard},
    {"GOING", OBJECT_FLAG_GOING, 'G', flag_going},
    {"HALTED", OBJECT_FLAG_HALTED, 'h', flag_wizard},
    {"IN_CHARACTER", OBJECT_FLAG_IN_CHARACTER, '#', flag_wizard},
    {"LIGHT", OBJECT_FLAG_LIGHT, 'l', flag_wizard},
    {"MONITOR", OBJECT_FLAG_MONITOR, 'M', flag_wizard},
    {"NO_COMMAND", OBJECT_FLAG_NO_COMMAND, 'n', flag_wizard},
    {"SAFE", OBJECT_FLAG_SAFE, 's', flag_wizard},
    {"SUSPECT", OBJECT_FLAG_SUSPECT, 'u', flag_wizard},
    {"TRANSPARENT", OBJECT_FLAG_TRANSPARENT, 't', flag_wizard},
    {"WIZARD", OBJECT_FLAG_WIZARD, 'W', flag_wizard_bit},
    {"XCODE", OBJECT_FLAG_XCODE, 'X', flag_xcode},
    {"ZOMBIE", OBJECT_FLAG_ZOMBIE, 'z', flag_wizard},
    {nullptr, OBJECT_FLAG_NONE, ' ', nullptr}};

static size_t flag_entry_count(void) {
  return (sizeof(FLAG_ENTRIES) / sizeof(*FLAG_ENTRIES)) - 1;
}

static const FlagEntry *flag_entry_at(size_t index) {
  return checked_storage_at_const(FLAG_ENTRIES, flag_entry_count(),
                                  sizeof(*FLAG_ENTRIES), index);
}

static const ObjectEntry OBJECT_TYPES[8] = {
    {"ROOM", 'R', CA_PUBLIC, OF_CONTENTS | OF_EXITS | OF_DROPTO | OF_HOME},
    {"THING", ' ', CA_PUBLIC,
     OF_CONTENTS | OF_LOCATION | OF_EXITS | OF_HOME | OF_SIBLINGS},
    {"EXIT", 'E', CA_PUBLIC, OF_SIBLINGS},
    {"PLAYER", 'P', CA_PUBLIC,
     OF_CONTENTS | OF_LOCATION | OF_EXITS | OF_HOME | OF_SIBLINGS},
    {"TYPE4", '+', CA_GOD, 0},
    {"GARBAGE", '-', CA_PUBLIC,
     OF_CONTENTS | OF_LOCATION | OF_EXITS | OF_HOME | OF_SIBLINGS},
    {"TYPE6", '#', CA_GOD, 0},
    {"TYPE7", '#', CA_GOD, 0}};

const ObjectEntry *object_type_entry(int type) {
  switch (type) {
  case OBJECT_TYPE_ROOM:
    return &OBJECT_TYPES[0];
  case OBJECT_TYPE_THING:
    return &OBJECT_TYPES[1];
  case OBJECT_TYPE_EXIT:
    return &OBJECT_TYPES[2];
  case OBJECT_TYPE_PLAYER:
    return &OBJECT_TYPES[3];
  case OBJECT_TYPE_INVALID:
    return &OBJECT_TYPES[4];
  case OBJECT_TYPE_GARBAGE:
    return &OBJECT_TYPES[5];
  case 6:
    return &OBJECT_TYPES[6];
  case OBJECT_TYPE_NOTYPE:
    return &OBJECT_TYPES[7];
  default:
    return &OBJECT_TYPES[4];
  }
}

void init_flagtab(WorldIndexes *indexes) {
  char buffer[SBUF_SIZE];
  hash_table_initialize(&indexes->flags, 100 * HASH_FACTOR);
  for (size_t index = 0; index < flag_entry_count(); index++) {
    const FlagEntry *flag = flag_entry_at(index);
    size_t name_length = strlen(flag->flagname);
    for (size_t name_index = 0; name_index < name_length; name_index++) {
      const char *input = checked_storage_at_const(flag->flagname, name_length,
                                                   sizeof(char), name_index);
      char *output =
          checked_storage_at(buffer, sizeof(buffer), sizeof(char), name_index);
      *output = ascii_to_lower(*input);
    }
    *(char *)checked_storage_at(buffer, sizeof(buffer), sizeof(char),
                                name_length) = '\0';
    hash_table_add_const(buffer, flag, &indexes->flags);
  }
}
void display_flagtab(EvaluationContext *evaluation, DbRef player) {
  char *buffer = alloc_lbuf("display_flagtab");
  char *out = buffer;
  safe_str("Flags:", buffer, &out);
  for (size_t index = 0; index < flag_entry_count(); index++) {
    const FlagEntry *flag = flag_entry_at(index);
    safe_chr(' ', buffer, &out);
    safe_str(flag->flagname, buffer, &out);
    safe_chr('(', buffer, &out);
    safe_chr(flag->flaglett, buffer, &out);
    safe_chr(')', buffer, &out);
  }
  *out = '\0';
  notify_checked(evaluation, player, player, buffer, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(buffer);
}
static bool flag_normalize_name(const char *name,
                                char buffer[static SBUF_SIZE]) {
  if (name == nullptr)
    return false;

  size_t length = strlen(name);
  if (length >= SBUF_SIZE)
    return false;
  for (size_t index = 0; index < length; index++) {
    const char *input =
        checked_storage_at_const(name, length, sizeof(char), index);
    char *output = checked_storage_at(buffer, SBUF_SIZE, sizeof(char), index);
    *output = ascii_to_lower(*input);
  }
  *(char *)checked_storage_at(buffer, SBUF_SIZE, sizeof(char), length) = '\0';
  return true;
}

const FlagEntry *find_flag(WorldIndexes *indexes, DbRef thing,
                           const char *flagname) {
  char normalized[SBUF_SIZE];

  (void)thing;
  if (!flag_normalize_name(flagname, normalized))
    return nullptr;
  return hash_table_find_const(normalized, &indexes->flags);
}

bool flag_alias_add(WorldIndexes *indexes, const char *alias,
                    const FlagEntry *flag) {
  char normalized[SBUF_SIZE];
  const FlagEntry *existing;

  if (flag == nullptr || !flag_normalize_name(alias, normalized))
    return false;
  existing = hash_table_find_const(normalized, &indexes->flags);
  if (existing != nullptr)
    return existing == flag;
  return hash_table_add_const(normalized, flag, &indexes->flags) == 0;
}
void flag_set(EvaluationContext *evaluation, WorldIndexes *indexes,
              DbRef target, DbRef player, char *name, int key) {
  bool clear = false;
  size_t offset = 0;
  size_t length = strlen(name);
  while (offset < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             name, length, sizeof(char), offset)))
    offset++;
  name = checked_mutable_string_suffix(name, offset);
  if (*name == '!') {
    clear = true;
    name = checked_mutable_string_suffix(name, 1);
  }
  length = strlen(name);
  offset = 0;
  while (offset < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             name, length, sizeof(char), offset)))
    offset++;
  name = checked_mutable_string_suffix(name, offset);
  if (!*name) {
    notify_checked(evaluation, player, player,
                   clear ? "You must specify a flag to clear."
                         : "You must specify a flag to set.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  const FlagEntry *flag = find_flag(indexes, target, name);
  if (!flag) {
    notify_checked(evaluation, player, player, "I don't understand that flag.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  if (!flag->handler(&(FlagChangeRequest){.evaluation = evaluation,
                                          .target = target,
                                          .player = player,
                                          .flag = flag->id,
                                          .clear = clear})) {
    notify_checked(evaluation, player, player, "Permission denied.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  if (!(key & SET_QUIET))
    notify_printf(evaluation, player, "%s - %s %s",
                  game_object_name(evaluation->world->database, target),
                  flag->flagname, clear ? "cleared." : "set.");
}
char *decode_flags(const DecodeFlagsRequest *request) {
  char *buffer = alloc_sbuf("decode_flags");
  char *out = buffer;
  *out = '\0';
  if (!is_good_obj(request->database, request->player)) {
    (void)string_copy_bounded(buffer, SBUF_SIZE, "#-2 ERROR");
    return buffer;
  }
  const ObjectEntry *object_type = object_type_entry(request->object_type);
  if (object_type->lett != ' ')
    safe_sb_chr(object_type->lett, buffer, &out);
  for (size_t index = 0; index < flag_entry_count(); index++) {
    const FlagEntry *flag = flag_entry_at(index);
    if (!object_flag_set_has(request->flags, flag->id))
      continue;
    safe_sb_chr(flag->flaglett, buffer, &out);
  }
  *out = '\0';
  return buffer;
}
char *flag_description(GameDatabase *database, DbRef target) {
  char *buffer = alloc_mbuf("flag_description");
  char *out = buffer;
  safe_mb_str("Type: ", buffer, &out);
  safe_mb_str(object_type_entry(typeof_obj(database, target))->name, buffer,
              &out);
  safe_mb_str(" Flags:", buffer, &out);
  for (size_t index = 0; index < flag_entry_count(); index++) {
    const FlagEntry *flag = flag_entry_at(index);
    if (game_object_has_flag(&(ObjectFlagRequest){
            .database = database, .object = target, .flag = flag->id})) {
      safe_mb_chr(' ', buffer, &out);
      safe_mb_str(flag->flagname, buffer, &out);
    }
  }
  *out = '\0';
  return buffer;
}

char *flags_description(GameDatabase *database, DbRef target) {
  char *buffer = alloc_mbuf("flags_description");
  char *out = buffer;

  safe_mb_str("Flags:", buffer, &out);
  for (size_t index = 0; index < flag_entry_count(); index++) {
    const FlagEntry *flag = flag_entry_at(index);
    if (game_object_has_flag(&(ObjectFlagRequest){
            .database = database, .object = target, .flag = flag->id})) {
      safe_mb_chr(' ', buffer, &out);
      safe_mb_str(flag->flagname, buffer, &out);
    }
  }
  *out = '\0';
  return buffer;
}

LbufText unparse_object_numonly(GameDatabase *database, DbRef target) {
  char *buffer = alloc_lbuf("unparse_object_numonly");
  if (target == NOTHING)
    (void)string_copy_bounded(buffer, LBUF_SIZE, "*NOTHING*");
  else if (target == HOME)
    (void)string_copy_bounded(buffer, LBUF_SIZE, "*HOME*");
  else if (!is_good_obj(database, target))
    (void)snprintf(buffer, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  else
    (void)snprintf(buffer, LBUF_SIZE, "%s(#%ld)",
                   game_object_name(database, target), target);
  return lbuf_text_take(buffer);
}
LbufText unparse_object(GameDatabase *database, EvaluationContext *evaluation,
                        DbRef player, DbRef target) {
  (void)evaluation;
  char *buffer = alloc_lbuf("unparse_object");
  if (target == NOTHING) {
    (void)string_copy_bounded(buffer, LBUF_SIZE, "*NOTHING*");
  } else if (target == HOME) {
    (void)string_copy_bounded(buffer, LBUF_SIZE, "*HOME*");
  } else if (!is_good_obj(database, target)) {
    (void)snprintf(buffer, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  } else if (is_examinable(database, player, target)) {
    char *flags = unparse_flags(database, player, target);
    (void)snprintf(buffer, LBUF_SIZE, "%s(#%ld%s%s)",
                   game_object_name(database, target), target,
                   *flags ? ":" : "", flags);
    free_sbuf(flags);
  } else {
    (void)string_copy_bounded(buffer, LBUF_SIZE,
                              game_object_name(database, target));
  }
  return lbuf_text_take(buffer);
}
bool convert_flags(EvaluationContext *evaluation, DbRef player, char *list,
                   ObjectFlagSet *flags, long *type) {
  *flags = (ObjectFlagSet){0};
  *type = OBJECT_TYPE_NOTYPE;
  size_t list_length = strlen(list);
  for (size_t character_index = 0; character_index < list_length;
       character_index++) {
    char *character =
        checked_storage_at(list, list_length, sizeof(char), character_index);
    bool handled = false;
    for (int index = 0; index < 8 && !handled; index++)
      if (object_type_entry(index)->lett == *character) {
        *type = index;
        handled = true;
      }
    for (size_t index = 0; index < flag_entry_count() && !handled; index++) {
      const FlagEntry *flag = flag_entry_at(index);
      if (flag->flaglett == *character) {
        object_flag_set_set(flags, flag->id, true);
        handled = true;
      }
    }
    if (!handled) {
      notify_printf(evaluation, player,
                    "%c: Flag unknown or not valid for specified object type",
                    *character);
      return false;
    }
  }
  return true;
}
