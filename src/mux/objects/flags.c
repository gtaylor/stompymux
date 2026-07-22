/* flags.c - object flag manipulation routines */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/world/world_context.h"

bool game_object_has_flag(GameDatabase *database, DbRef object,
                          ObjectFlag flag) {
  const GameObject *game_object = game_database_object(database, object);

  switch (flag) {
  case OBJECT_FLAG_ANSI:
    return game_object->has_ansi_flag;
  case OBJECT_FLAG_ANSIMAP:
    return game_object->has_ansimap_flag;
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
  case OBJECT_FLAG_QUIET:
    return game_object->has_quiet_flag;
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

void game_object_set_flag(GameDatabase *database, DbRef object, ObjectFlag flag,
                          bool value) {
  GameObject *game_object = game_database_object(database, object);

  switch (flag) {
  case OBJECT_FLAG_ANSI:
    game_object->has_ansi_flag = value;
    break;
  case OBJECT_FLAG_ANSIMAP:
    game_object->has_ansimap_flag = value;
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
  case OBJECT_FLAG_QUIET:
    game_object->has_quiet_flag = value;
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
    game_object_set_flag(database, object, flag, false);
}

void game_object_flags_copy(GameDatabase *database, DbRef object,
                            ObjectFlagSet *flags) {
  for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++)
    flags->values[flag] = game_object_has_flag(database, object, flag);
}

bool object_flag_set_has(const ObjectFlagSet *flags, ObjectFlag flag) {
  return flag > OBJECT_FLAG_NONE && flag < OBJECT_FLAG_COUNT &&
         flags->values[flag];
}

void object_flag_set_set(ObjectFlagSet *flags, ObjectFlag flag, bool value) {
  if (flag > OBJECT_FLAG_NONE && flag < OBJECT_FLAG_COUNT)
    flags->values[flag] = value;
}

bool is_good_obj(GameDatabase *database, DbRef x) {
  return x >= 0 && x < database->top &&
         typeof_obj(database, x) != OBJECT_TYPE_INVALID &&
         typeof_obj(database, x) != OBJECT_TYPE_NOTYPE;
}

bool is_safe(GameDatabase *database, const ServerConfiguration *configuration,
             DbRef x, DbRef p) {
  (void)configuration;
  (void)p;
  return is_player(database, x) ||
         game_object_has_flag(database, x, OBJECT_FLAG_SAFE);
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
  const unsigned char mask = (unsigned char)(1U << (x & 7));
  database->markbits->chunk[x >> 3] =
      (char)((unsigned char)database->markbits->chunk[x >> 3] | mask);
}
void unmark(GameDatabase *database, DbRef x) {
  const unsigned char mask = (unsigned char)(1U << (x & 7));
  database->markbits->chunk[x >> 3] =
      (char)((unsigned char)database->markbits->chunk[x >> 3] &
             (unsigned char)~mask);
}
bool is_marked(GameDatabase *database, DbRef x) {
  return ((unsigned char)database->markbits->chunk[x >> 3] &
          (unsigned char)(1U << (x & 7))) != 0;
}
void unmark_all(GameDatabase *database) {
  for (DbRef index = 0; index < ((database->top + 7) >> 3); index++)
    database->markbits->chunk[index] = 0;
}

bool see_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
              long f) {
  (void)x;
  (void)a;
  (void)f;
  return is_wizard(evaluation->world->database, p);
}
bool see_attr_explicit(GameDatabase *database, DbRef p, DbRef x, Attribute *a,
                       long f) {
  (void)x;
  (void)a;
  (void)f;
  return is_wizard(database, p);
}
bool set_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
              long f) {
  return see_attr(evaluation, p, x, a, f);
}
bool read_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
               long f) {
  return see_attr(evaluation, p, x, a, f);
}
bool write_attr(EvaluationContext *evaluation, DbRef p, DbRef x, Attribute *a,
                long f) {
  return see_attr(evaluation, p, x, a, f);
}

static bool flag_any(EvaluationContext *evaluation, DbRef target, DbRef player,
                     ObjectFlag flag, bool clear) {
  (void)player;
  game_object_set_flag(evaluation->world->database, target, flag, !clear);
  return true;
}
static bool flag_god(EvaluationContext *evaluation, DbRef target, DbRef player,
                     ObjectFlag flag, bool clear) {
  return is_god(evaluation->world->database, player) &&
         flag_any(evaluation, target, player, flag, clear);
}
static bool flag_wizard(EvaluationContext *evaluation, DbRef target,
                        DbRef player, ObjectFlag flag, bool clear) {
  return (is_wizard(evaluation->world->database, player) ||
          is_god(evaluation->world->database, player)) &&
         flag_any(evaluation, target, player, flag, clear);
}
static bool flag_wizard_bit(EvaluationContext *evaluation, DbRef target,
                            DbRef player, ObjectFlag flag, bool clear) {
  if (!is_god(evaluation->world->database, player))
    return false;
  if (is_god(evaluation->world->database, target) && clear) {
    notify(evaluation, player, "You cannot make yourself mortal.");
    return false;
  }
  return flag_any(evaluation, target, player, flag, clear);
}
static bool flag_going(EvaluationContext *evaluation, DbRef target,
                       DbRef player, ObjectFlag flag, bool clear) {
  if (is_going(evaluation->world->database, target) && clear &&
      !is_player(evaluation->world->database, target))
    return flag_any(evaluation, target, player, flag, clear);
  return is_god(evaluation->world->database, player) &&
         flag_any(evaluation, target, player, flag, clear);
}

static bool flag_xcode(EvaluationContext *evaluation, DbRef target,
                       DbRef player, ObjectFlag flag, bool clear) {
  if (clear && is_xcode(evaluation->world->database, target) &&
      !is_god(evaluation->world->database, player))
    return false;
  return flag_wizard(evaluation, target, player, flag, clear);
}

FlagEntry gen_flags[] = {
    {"ANSI", OBJECT_FLAG_ANSI, 'X', flag_wizard},
    {"ANSIMAP", OBJECT_FLAG_ANSIMAP, 'P', flag_wizard},
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
    {"QUIET", OBJECT_FLAG_QUIET, 'Q', flag_wizard},
    {"SAFE", OBJECT_FLAG_SAFE, 's', flag_wizard},
    {"SUSPECT", OBJECT_FLAG_SUSPECT, 'u', flag_wizard},
    {"TRANSPARENT", OBJECT_FLAG_TRANSPARENT, 't', flag_wizard},
    {"WIZARD", OBJECT_FLAG_WIZARD, 'W', flag_wizard_bit},
    {"XCODE", OBJECT_FLAG_XCODE, 'X', flag_xcode},
    {"ZOMBIE", OBJECT_FLAG_ZOMBIE, 'z', flag_wizard},
    {nullptr, OBJECT_FLAG_NONE, ' ', nullptr}};

ObjectEntry object_types[8] = {
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

void init_flagtab(WorldIndexes *indexes) {
  char *buffer = alloc_sbuf("init_flagtab");
  hash_table_initialize(&indexes->flags, 100 * HASH_FACTOR);
  for (FlagEntry *flag = gen_flags; flag->flagname; flag++) {
    char *out = buffer;
    for (const char *in = flag->flagname; *in; in++, out++)
      *out = ToLower(*in);
    *out = '\0';
    hash_table_add(buffer, (int *)flag, &indexes->flags);
  }
  free_sbuf(buffer);
}
void display_flagtab(EvaluationContext *evaluation, DbRef player) {
  char *buffer = alloc_lbuf("display_flagtab");
  char *out = buffer;
  safe_str("Flags:", buffer, &out);
  for (FlagEntry *flag = gen_flags; flag->flagname; flag++) {
    safe_chr(' ', buffer, &out);
    safe_str(flag->flagname, buffer, &out);
    safe_chr('(', buffer, &out);
    safe_chr(flag->flaglett, buffer, &out);
    safe_chr(')', buffer, &out);
  }
  *out = '\0';
  notify(evaluation, player, buffer);
  free_lbuf(buffer);
}
FlagEntry *find_flag(WorldIndexes *indexes, DbRef thing, char *flagname) {
  (void)thing;
  for (char *character = flagname; *character; character++)
    *character = ToLower(*character);
  return (FlagEntry *)hash_table_find(flagname, &indexes->flags);
}
void flag_set(EvaluationContext *evaluation, WorldIndexes *indexes,
              DbRef target, DbRef player, char *name, int key) {
  bool clear = false;
  while (*name && isspace(*name))
    name++;
  if (*name == '!') {
    clear = true;
    name++;
  }
  while (*name && isspace(*name))
    name++;
  if (!*name) {
    notify(evaluation, player,
           clear ? "You must specify a flag to clear."
                 : "You must specify a flag to set.");
    return;
  }
  FlagEntry *flag = find_flag(indexes, target, name);
  if (!flag) {
    notify(evaluation, player, "I don't understand that flag.");
    return;
  }
  if (!flag->handler(evaluation, target, player, flag->id, clear)) {
    notify(evaluation, player, "Permission denied.");
    return;
  }
  if (!(key & SET_QUIET) && !is_quiet(evaluation->world->database, player))
    notify_printf(evaluation, player, "%s - %s %s",
                  game_object_name(evaluation->world->database, target),
                  flag->flagname, clear ? "cleared." : "set.");
}
char *decode_flags(GameDatabase *database, DbRef player, int type,
                   const ObjectFlagSet *flags) {
  char *buffer = alloc_sbuf("decode_flags");
  char *out = buffer;
  *out = '\0';
  if (!is_good_obj(database, player)) {
    StringCopy(buffer, "#-2 ERROR");
    return buffer;
  }
  if (object_types[type].lett != ' ')
    safe_sb_chr(object_types[type].lett, buffer, &out);
  for (FlagEntry *flag = gen_flags; flag->flagname; flag++) {
    if (!object_flag_set_has(flags, flag->id))
      continue;
    safe_sb_chr(flag->flaglett, buffer, &out);
  }
  *out = '\0';
  return buffer;
}
bool has_flag(WorldContext *world, DbRef player, DbRef target, char *name) {
  FlagEntry *flag = find_flag(world->indexes, target, name);
  (void)player;
  return flag && game_object_has_flag(world->database, target, flag->id);
}
char *flag_description(GameDatabase *database, DbRef player, DbRef target) {
  char *buffer = alloc_mbuf("flag_description");
  char *out = buffer;
  safe_mb_str("Type: ", buffer, &out);
  safe_mb_str(object_types[typeof_obj(database, target)].name, buffer, &out);
  safe_mb_str(" Flags:", buffer, &out);
  (void)player;
  for (FlagEntry *flag = gen_flags; flag->flagname; flag++)
    if (game_object_has_flag(database, target, flag->id)) {
      safe_mb_chr(' ', buffer, &out);
      safe_mb_str(flag->flagname, buffer, &out);
    }
  *out = '\0';
  return buffer;
}
char *unparse_object_numonly(GameDatabase *database, DbRef target) {
  char *buffer = alloc_lbuf("unparse_object_numonly");
  if (target == NOTHING)
    StringCopy(buffer, "*NOTHING*");
  else if (target == HOME)
    StringCopy(buffer, "*HOME*");
  else if (!is_good_obj(database, target))
    snprintf(buffer, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  else
    snprintf(buffer, LBUF_SIZE, "%s(#%ld)", game_object_name(database, target),
             target);
  return buffer;
}
char *unparse_object(GameDatabase *database, EvaluationContext *evaluation,
                     DbRef player, DbRef target) {
  (void)evaluation;
  char *buffer = alloc_lbuf("unparse_object");
  if (target == NOTHING)
    StringCopy(buffer, "*NOTHING*");
  else if (target == HOME)
    StringCopy(buffer, "*HOME*");
  else if (!is_good_obj(database, target))
    snprintf(buffer, LBUF_SIZE, "*ILLEGAL*(#%ld)", target);
  else if (is_examinable(database, player, target)) {
    char *flags = unparse_flags(database, player, target);
    snprintf(buffer, LBUF_SIZE, "%s(#%ld%s%s)",
             game_object_name(database, target), target, *flags ? ":" : "",
             flags);
    free_sbuf(flags);
  } else
    StringCopy(buffer, game_object_name(database, target));
  return buffer;
}
bool convert_flags(EvaluationContext *evaluation, DbRef player, char *list,
                   ObjectFlagSet *flags, long *type) {
  *flags = (ObjectFlagSet){0};
  *type = OBJECT_TYPE_NOTYPE;
  for (char *character = list; *character; character++) {
    bool handled = false;
    for (int index = 0; index < 8 && !handled; index++)
      if (object_types[index].lett == *character) {
        *type = index;
        handled = true;
      }
    for (FlagEntry *flag = gen_flags; flag->flagname && !handled; flag++)
      if (flag->flaglett == *character) {
        object_flag_set_set(flags, flag->id, true);
        handled = true;
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
