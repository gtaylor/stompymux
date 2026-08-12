#include "mux/commands/macro.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_helpers.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/commac.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h" // IWYU pragma: keep
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h" // IWYU pragma: keep
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/stringutil.h"
#include "mux/support/utf8.h"
#include "mux/support/validation.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
static MacroSet *macro_registry_storage_item(const MacroRegistry *registry,
                                             size_t index) {
  return *(MacroSet *const *)checked_storage_at_const(
      (const void *)registry->sets, (size_t)registry->capacity,
      sizeof(*registry->sets), index);
}
static char *macro_string_storage_item(const MacroSet *set, size_t index) {
  return *(char *const *)checked_storage_at_const((const void *)set->string,
                                                  (size_t)set->macro_capacity,
                                                  sizeof(*set->string), index);
}
static int *commac_macro_slot(struct Commac *commac, size_t index) {
  return checked_storage_at(commac->macros,
                            sizeof(commac->macros) / sizeof(commac->macros[0]),
                            sizeof(*commac->macros), index);
}
static int commac_macro_item(const struct Commac *commac, size_t index) {
  return *(const int *)checked_storage_at_const(
      commac->macros, sizeof(commac->macros) / sizeof(commac->macros[0]),
      sizeof(*commac->macros), index);
}
static bool is_valid_macro_index(const MacroRegistry *registry, int index) {
  return index >= 0 && index < registry->count;
}
static void macro_notify(MatchContext *m, DbRef p, const char *text) {
  notify_checked(m->evaluation, p, p, text, MSG_ME_ALL | MSG_F_DOWN);
}
MACENT macro_table[] = {{"add", do_add_macro},       {"clear", do_clear_macro},
                        {"chmod", do_chmod_macro},   {"chown", do_chown_macro},
                        {"create", do_create_macro}, {"def", do_def_macro},
                        {"del", do_del_macro},       {"name", do_desc_macro},
                        {"chslot", do_edit_macro},   {"ex", do_ex_macro},
                        {"gex", do_gex_macro},       {"glist", do_list_macro},
                        {"list", do_status_macro},   {"undef", do_undef_macro},
                        {(char *)nullptr, nullptr}};
static size_t macro_command_count(void) {
  return sizeof(macro_table) / sizeof(macro_table[0]) - 1;
}
static MACENT *macro_command_at(size_t index) {
  return checked_storage_at(macro_table, macro_command_count(),
                            sizeof(*macro_table), index);
}
void init_mactab(CommandRegistry *commands) {
  hash_table_initialize(&commands->macros, 5 * HASH_FACTOR);
  for (size_t index = 0; index < macro_command_count(); index++) {
    MACENT *mp = macro_command_at(index);
    hash_table_add(mp->cmdname, (int *)mp, &commands->macros);
  }
}
int do_macro(MatchContext *match, CommandRegistry *commands,
             MacroRegistry *registry, DbRef player, char *in, char **out) {
  char *s;
  char *cmd;
  MACENT *mp;
  char *old;
  cmd = checked_mutable_string_suffix(in, 1);
  if (!is_player(match->evaluation->world->database, player)) {
    macro_notify(match, player, "MACRO: Only players may use macro_sets.");
    return 0;
  }
  old = alloc_lbuf("do_macro");
  string_copy(old, in);
  const size_t COMMAND_LENGTH = strlen(cmd);
  size_t command_end = 0;
  while (command_end < COMMAND_LENGTH &&
         *(const char *)checked_storage_at_const(
             cmd, COMMAND_LENGTH + 1, sizeof(char), command_end) != ' ')
    command_end++;
  s = checked_storage_at(cmd, COMMAND_LENGTH + 1, sizeof(char), command_end);
  if (*s == ' ') {
    *s = 0;
    s = checked_storage_at(cmd, COMMAND_LENGTH + 1, sizeof(char),
                           command_end + 1);
  }
  mp = (MACENT *)hash_table_find(cmd, &commands->macros);
  if (mp != nullptr) {
    (mp->handler)(match, registry, player, s);
    free_lbuf(old);
    return 0;
  }
  *out = do_process_macro(&(MacroExpansionRequest){
      .registry = registry, .player = player, .input = in, .arguments = s});
  if (*out) {
    free_lbuf(old);
    return 1;
  }
  string_copy(in, old);
  free_lbuf(old);
  return 2; /*
             * return any value > 1, and command * * *
             * processing will
             */
  /*
   * continue
   */
}
void do_list_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                   char *s) {
  int i;
  int notified = 0;
  MacroSet *m;
  char *unparse;
  for (i = 0; i < registry->count; i++) {
    m = macro_registry_item(registry, (size_t)i);
    if (can_read_macros(match->evaluation->world->database, player, m)) {
      if (!notified) {
        macro_notify(match, player,
                     "Num  Description                         Owner         "
                     "            LRW");
        notified = 1;
      }
      unparse = unparse_object(match->evaluation->world->database,
                               match->evaluation, player, m->player);
      notify_printf(match->evaluation, player, "%-4d %-35.35s %-24.24s  %c%c%c",
                    i, m->desc, unparse, m->status & MACRO_L ? 'L' : '-',
                    m->status & MACRO_R ? 'R' : '-',
                    m->status & MACRO_W ? 'W' : '-');
      free_lbuf(unparse);
    }
  }
  if (!notified)
    macro_notify(match, player, "MACRO: There are no macro sets you can read.");
}
void do_add_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *s) {
  int first;
  int set;
  MacroSet *m;
  struct Commac *c;
  int i;
  c = get_commac(registry->channels, player);
  first = -1;
  for (i = 0; i < 5 && first < 0; i++)
    if (commac_macro_item(c, (size_t)i) == -1)
      first = i;
  if (first < 0) {
    macro_notify(match, player,
                 "MACRO: Sorry, you already have 5 sets defined on you.");
  } else if (is_number(s)) {
    set = clamped_atoi(s);
    if (set >= 0 && set < registry->count) {
      m = macro_registry_item(registry, (size_t)set);
      if (can_read_macros(match->evaluation->world->database, player, m)) {
        *commac_macro_slot(c, (size_t)first) = set;
        notify_printf(match->evaluation, player,
                      "MACRO: Macro set %d added in the %d slot.", set, first);
      } else {
        macro_notify(match, player, "MACRO: Permission denied.");
      }
    } else {
      macro_notify(match, player, "MACRO: That macro set does not exist.");
      return;
    }
  } else {
    macro_notify(match, player,
                 "MACRO: What set do you want to add to your macro system?");
  }
}
void do_del_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *s) {
  (void)registry;
  struct Commac *c;
  int set;
  c = get_commac(registry->channels, player);
  if (is_number(s)) {
    set = clamped_atoi(s);
    if (set >= 0 && set < 5 && commac_macro_item(c, (size_t)set) >= 0) {
      *commac_macro_slot(c, (size_t)set) = -1;
      notify_printf(match->evaluation, player, "MACRO: Macro slot %d cleared.",
                    set);
      if (set == c->curmac) {
        c->curmac = -1;
        macro_notify(match, player,
                     "MACRO: Deleted current slot, resetting to none.");
      }
    } else {
      macro_notify(match, player, "MACRO: That is not a legal macro slot.");
    }
  } else {
    notify_checked(
        match->evaluation, player, player,
        "MACRO: What set did you want to delete from your macro system?",
        MSG_ME_ALL | MSG_F_DOWN);
  }
}
void do_desc_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                   char *s) {
  MacroSet *m;
  m = get_macro_set(
      &(MacroSetRequest){.registry = registry, .player = player, .slot = -1});
  if (m) {
    free(m->desc);
    m->desc = malloc(strlen(s) + 1);
    string_copy(m->desc, s);
    notify_printf(match->evaluation, player,
                  "MACRO: Current slot description to %s.", s);
  } else {
    macro_notify(match, player, "MACRO: You have no current slot set.");
  }
}
void do_chmod_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *s) {
  MacroSet *m;
  int sign;
  m = get_macro_set(
      &(MacroSetRequest){.registry = registry, .player = player, .slot = -1});
  if (m) {
    if ((m->player != player) &&
        !is_wizard(match->evaluation->world->database, player)) {
      macro_notify(match, player, "MACRO: Permission denied.");
      return;
    }
    if (*s == '!') {
      sign = 0;
      s = checked_mutable_string_suffix(s, 1);
    } else {
      sign = 1;
    }
    switch (*s) {
    case 'L':
    case 'l':
      if (sign) {
        m->status |= MACRO_L;
        notify_checked(
            match->evaluation, player, player,
            "MACRO: Default Macro Slot is now locked and unwritable.",
            MSG_ME_ALL | MSG_F_DOWN);
      } else {
        m->status &= ~MACRO_L;
        macro_notify(match, player,
                     "MACRO: Default Macro Slot is now unlocked.");
      }
      break;
    case 'R':
    case 'r':
      if (sign) {
        m->status |= MACRO_R;
        macro_notify(match, player,
                     "MACRO: Default Macro Slot set to be readable by others");
      } else {
        m->status &= ~MACRO_R;
        notify_checked(
            match->evaluation, player, player,
            "MACRO: Default Macro Slot set to be not readable by others",
            MSG_ME_ALL | MSG_F_DOWN);
      }
      break;
    case 'W':
    case 'w':
      if (sign) {
        m->status |= MACRO_W;
        macro_notify(match, player,
                     "MACRO: Default Macro Slot set to be writable by others");
      } else {
        m->status &= ~MACRO_W;
        notify_checked(
            match->evaluation, player, player,
            "MACRO: Default Macro Slot set to be not writable by others",
            MSG_ME_ALL | MSG_F_DOWN);
      }
      break;
    default:
      macro_notify(match, player,
                   "MACRO: Sorry, unknown mode.  Legal modes are: L R W");
    }
  } else {
    macro_notify(match, player, "MACRO: You have no current slot set.");
  }
}
void do_gex_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *s) {
  MacroSet *m;
  int which;
  int i;
  char buffer[LBUF_SIZE];
  if (!s || !*s) {
    macro_notify(match, player, "MACRO: You need to specify a macro set.");
    return;
  }
  if (is_number(s)) {
    which = clamped_atoi(s);
    if ((which >= registry->count) || (which < 0) || (registry->count == 0)) {
      notify_printf(match->evaluation, player,
                    "MACRO: Illegal Macro Set.  Macros go from 0 to %d.",
                    registry->count - 1);
      return;
    }
    m = macro_registry_item(registry, (size_t)which);
  } else {
    macro_notify(match, player, "MACRO: I do not see that set here.");
    return;
  }
  if (m && can_read_macros(match->evaluation->world->database, player, m)) {
    notify_printf(match->evaluation, player, "Macro Definitions for %s",
                  m->desc);
    for (i = 0; i < m->macro_count; i++) {
      (void)snprintf(buffer, sizeof(buffer), "  %-5.5s: %s",
                     macro_alias_at(m, (size_t)i),
                     macro_string_item(m, (size_t)i));
      macro_notify(match, player, buffer);
    }
  } else {
    macro_notify(match, player, "MACRO: Permission denied.");
  }
}
void do_edit_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                   char *s) {
  struct Commac *c;
  int set;
  c = get_commac(registry->channels, player);
  if (is_number(s)) {
    set = clamped_atoi(s);
    if (set >= 0 && set < 5 &&
        is_valid_macro_index(registry, commac_macro_item(c, (size_t)set))) {
      c->curmac = set;
      notify_printf(match->evaluation, player, "MACRO: Current slot set to %d.",
                    set);
    } else {
      macro_notify(match, player, "MACRO: That is not a legal macro slot.");
    }
  } else {
    macro_notify(match, player,
                 "MACRO: What slot did you want to make current?");
  }
}
void do_status_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                     char *s) {
  int i;
  struct Commac *c;
  MacroSet *m;
  char *unparse;
  c = get_commac(registry->channels, player);
  macro_notify(match, player,
               "#: Num  Description                         Owner            "
               "         LRW");
  for (i = 0; i < 5; i++) {
    const int MACRO_INDEX = commac_macro_item(c, (size_t)i);
    if (MACRO_INDEX >= 0)
      if (!(is_valid_macro_index(registry, MACRO_INDEX)))
        notify_printf(match->evaluation, player, "%d: INVALID MACRO SET!", i);
      else {
        m = macro_registry_item(registry, (size_t)MACRO_INDEX);
        unparse = unparse_object(match->evaluation->world->database,
                                 match->evaluation, player, m->player);
        notify_printf(
            match->evaluation, player, "%d: %-4d %-35.35s %-24.24s  %c%c%c", i,
            MACRO_INDEX, m->desc, unparse, m->status & MACRO_L ? 'L' : '-',
            m->status & MACRO_R ? 'R' : '-', m->status & MACRO_W ? 'W' : '-');
        free_lbuf(unparse);
      }
    else
      notify_printf(match->evaluation, player, "%d:", i);
  }
  notify_printf(match->evaluation, player, "Current Macro Slot: %d", c->curmac);
}
void do_ex_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                 char *s) {
  MacroSet *m;
  int which;
  int i;
  char buffer[LBUF_SIZE];
  if (is_number(s)) {
    which = clamped_atoi(s);
    m = get_macro_set(&(MacroSetRequest){
        .registry = registry, .player = player, .slot = which});
  } else {
    m = get_macro_set(
        &(MacroSetRequest){.registry = registry, .player = player, .slot = -1});
  }
  if (m) {
    notify_printf(match->evaluation, player, "Macro Definitions for %s",
                  m->desc);
    for (i = 0; i < m->macro_count; i++) {
      (void)snprintf(buffer, sizeof(buffer), "  %-5.5s: %s",
                     macro_alias_at(m, (size_t)i),
                     macro_string_item(m, (size_t)i));
      macro_notify(match, player, buffer);
    }
  } else {
    macro_notify(match, player, "MACRO: Illegal macro set to examine.");
  }
}
void do_chown_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *cmd) {
  MacroSet *m;
  DbRef thing;
  char *unparse;
  m = get_macro_set(
      &(MacroSetRequest){.registry = registry, .player = player, .slot = -1});
  thing = match_thing(match, player, cmd);
  if (thing == NOTHING) {
    macro_notify(match, player, "MACRO: I do not see that here.");
    return;
  }
  if (!m) {
    macro_notify(match, player, "MACRO: No current active macro.");
    return;
  }
  if (!is_wizard(match->evaluation->world->database, player)) {
    macro_notify(match, player, "MACRO: Sorry, command limited to Wizards.");
    return;
  }
  m->player = (int)thing;
  unparse = unparse_object(match->evaluation->world->database,
                           match->evaluation, player, thing);
  notify_printf(match->evaluation, player, "MACRO: Macro %s chowned to %s.",
                m->desc, unparse);
  free_lbuf(unparse);
}
void clear_macro_set(MacroRegistry *registry, int set) {
  MacroSet *m;
  struct Commac *c;
  int i;
  int j;
  if (is_valid_macro_index(registry, set)) {
    m = macro_registry_item(registry, (size_t)set);
    for (i = 0; i < m->macro_count; i++) {
      free(macro_string_item(m, (size_t)i));
    }
    free(m->alias);
    free((void *)m->string);
    free(m);
    registry->count--;
    for (i = set; i < registry->count; i++)
      *macro_registry_slot(registry, (size_t)i) =
          macro_registry_storage_item(registry, (size_t)i + 1);
    *macro_registry_slot(registry, (size_t)i) = nullptr;
  }
  for (i = 0; i < COMMAC_BUCKET_COUNT; i++) {
    c = channel_registry_bucket_at(registry->channels, (size_t)i);
    while (c) {
      for (j = 0; j < 5; j++) {
        int *macro = commac_macro_slot(c, (size_t)j);
        if (*macro == set) {
          *macro = -1;
          if (c->curmac == j)
            c->curmac = -1;
        } else if (*macro > set) {
          (*macro)--;
        }
      }
      c = c->next;
    }
  }
}
void do_clear_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *s) {
  int set;
  MacroSet *m;
  struct Commac *c;
  c = get_commac(registry->channels, player);
  if (c->curmac == -1) {
    macro_notify(match, player,
                 "MACRO: You are not currently editing a macro set.");
    return;
  }
  if (commac_macro_item(c, (size_t)c->curmac) == -1) {
    macro_notify(match, player, "MACRO: That is not a valid macro set.");
    return;
  }
  set = commac_macro_item(c, (size_t)c->curmac);
  m = is_valid_macro_index(registry, set)
          ? macro_registry_item(registry, (size_t)set)
          : nullptr;
  if (is_valid_macro_index(registry, set)) {
    if ((player != m->player) &&
        !is_wizard(match->evaluation->world->database, player)) {
      macro_notify(match, player,
                   "MACRO: You may only CLEAR your own macro sets.");
      return;
    }
    if ((player == m->player) && (m->status & MACRO_L)) {
      macro_notify(match, player, "MACRO: Sorry, that macro set is locked.");
      return;
    }
  }
  notify_printf(match->evaluation, player, "MACRO: Clearing macro set %d: %s.",
                set,
                is_valid_macro_index(registry, set) ? m->desc : "Nonexistent");
  clear_macro_set(registry, set);
}
void do_def_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                  char *cmd) {
  int i;
  int j;
  int where;
  MacroSet *m;
  char *alias;
  char *s;
  char buffer[LBUF_SIZE];
  char **ns;
  char *na;
  m = get_macro_set(
      &(MacroSetRequest){.registry = registry, .player = player, .slot = -1});
  if (!m) {
    macro_notify(match, player, "MACRO: No current set.");
    return;
  }
  if (!can_write_macros(player, m)) {
    macro_notify(match, player, "MACRO: Permission denied.");
    return;
  }
  char *input = cmd;
  const size_t INPUT_LENGTH = strlen(input);
  size_t offset = 0;
  while (offset < INPUT_LENGTH &&
         *(const char *)checked_storage_at_const(input, INPUT_LENGTH + 1,
                                                 sizeof(char), offset) == ' ') {
    *(char *)checked_storage_at(input, INPUT_LENGTH + 1, sizeof(char), offset) =
        '\0';
    offset++;
  }
  alias = checked_storage_at(input, INPUT_LENGTH + 1, sizeof(char), offset);
  while (offset < INPUT_LENGTH) {
    const char CHARACTER = *(const char *)checked_storage_at_const(
        input, INPUT_LENGTH + 1, sizeof(char), offset);
    if (CHARACTER == ' ' || CHARACTER == '=')
      break;
    offset++;
  }
  while (offset < INPUT_LENGTH &&
         *(const char *)checked_storage_at_const(input, INPUT_LENGTH + 1,
                                                 sizeof(char), offset) == ' ') {
    *(char *)checked_storage_at(input, INPUT_LENGTH + 1, sizeof(char), offset) =
        '\0';
    offset++;
  }
  if (*(const char *)checked_storage_at_const(input, INPUT_LENGTH + 1,
                                              sizeof(char), offset) != '=') {
    macro_notify(match, player,
                 "MACRO: You must specify an = in your macro definition");
    return;
  }
  *(char *)checked_storage_at(input, INPUT_LENGTH + 1, sizeof(char), offset) =
      0;
  offset++;
  while (offset < INPUT_LENGTH &&
         *(const char *)checked_storage_at_const(input, INPUT_LENGTH + 1,
                                                 sizeof(char), offset) == ' ') {
    *(char *)checked_storage_at(input, INPUT_LENGTH + 1, sizeof(char), offset) =
        0;
    offset++;
  }
  cmd = checked_storage_at(input, INPUT_LENGTH + 1, sizeof(char), offset);
  s = cmd;
  if (!*s) {
    macro_notify(match, player,
                 "MACRO: You must specify a string to substitute for.");
    return;
  }
  if (!*alias || strlen(alias) > 4) {
    macro_notify(match, player,
                 "MACRO: Please use an alias from 1 to 4 characters long.");
    return;
  }
  if (!utf8_is_printable_ascii(alias, strlen(alias))) {
    notify_checked(
        match->evaluation, player, player,
        "MACRO: Aliases must contain only printable ASCII characters.",
        MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  for (j = 0; j < m->macro_count &&
              (strcasecmp(alias, macro_alias_at(m, (size_t)j)) > 0);
       j++)
    ;
  if (j < m->macro_count && !strcasecmp(alias, macro_alias_at(m, (size_t)j))) {
    macro_notify(match, player,
                 "MACRO: That alias is already defined in this set.");
    (void)snprintf(buffer, sizeof(buffer), "%-4.4s:%s",
                   macro_alias_at(m, (size_t)j),
                   macro_string_item(m, (size_t)j));
    macro_notify(match, player, buffer);
    return;
  }
  if (m->macro_count >= m->macro_capacity) {
    m->macro_capacity += 10;
    na = malloc(5 * (size_t)m->macro_capacity);
    ns = (char **)malloc(sizeof(char *) * (size_t)m->macro_capacity);
    for (i = 0; i < m->macro_count; i++) {
      string_copy(checked_storage_at(na, (size_t)m->macro_capacity * 5,
                                     sizeof(char), (size_t)i * 5),
                  macro_alias_at(m, (size_t)i));
      *(char **)checked_storage_at((void *)ns, (size_t)m->macro_capacity,
                                   sizeof(*ns), (size_t)i) =
          macro_string_item(m, (size_t)i);
    }
    free(m->alias);
    free((void *)m->string);
    m->alias = na;
    m->string = ns;
  }
  where = m->macro_count++;
  for (i = where; i > j; i--) {
    string_copy(macro_alias_at(m, (size_t)i), macro_alias_at(m, (size_t)i - 1));
    *macro_string_slot(m, (size_t)i) = macro_string_item(m, (size_t)i - 1);
  }
  where = j;
  string_copy(macro_alias_at(m, (size_t)where), alias);
  *macro_string_slot(m, (size_t)where) = malloc(strlen(s) + 1);
  string_copy(macro_string_item(m, (size_t)where), s);
  (void)snprintf(buffer, sizeof(buffer), "MACRO: Macro %s:%s defined.", alias,
                 s);
  macro_notify(match, player, buffer);
}
void do_undef_macro(MatchContext *match, MacroRegistry *registry, DbRef player,
                    char *cmd) {
  int i;
  MacroSet *m;
  m = get_macro_set(
      &(MacroSetRequest){.registry = registry, .player = player, .slot = -1});
  if (!m || !can_write_macros(player, m)) {
    macro_notify(match, player, "MACRO: Permission denied.");
    return;
  }
  for (i = 0; i < m->macro_count; i++) {
    if (!strcmp(macro_alias_at(m, (size_t)i), cmd)) {
      free(macro_string_item(m, (size_t)i));
      m->macro_count--;
      for (; i < m->macro_count; i++) {
        string_copy(macro_alias_at(m, (size_t)i),
                    macro_alias_at(m, (size_t)i + 1));
        *macro_string_slot(m, (size_t)i) =
            macro_string_storage_item(m, (size_t)i + 1);
      }
      macro_notify(match, player, "MACRO: Macro deleted from set.");
      return;
    }
  }
  macro_notify(match, player, "MACRO: That macro is not in this set.");
}
char *do_process_macro(const MacroExpansionRequest *request) {
  MacroRegistry *registry = request->registry;
  DbRef player = request->player;
  char *in = request->input;
  char *s = request->arguments;
  char *tar;
  char *next;
  MacroSet *m;
  int first;
  int last;
  int current = 0;
  int dir;
  int i;
  struct Commac *c;
  char *buff;
  c = get_commac(registry->channels, player);
  buff = alloc_lbuf("do_process_macro");
  buff[0] = '\0'; /*
                   * End the string
                   */
  for (i = 0; i < 5; i++) {
    const int MACRO_INDEX = commac_macro_item(c, (size_t)i);
    if (is_valid_macro_index(registry, MACRO_INDEX)) {
      m = macro_registry_item(registry, (size_t)MACRO_INDEX);
      if (m->macro_count > 0) {
        first = 0;
        last = m->macro_count - 1;
        dir = 1;
        next = checked_mutable_string_suffix(in, 1);
        while (dir && (first <= last)) {
          current = (first + last) / 2;
          dir = strcmp(next, macro_alias_at(m, (size_t)current));
          if (dir < 0)
            last = current - 1;
          else
            first = current + 1;
        }
        if (!dir) {
          tar = macro_string_item(m, (size_t)current);
          const size_t REPLACEMENT_LENGTH = strlen(tar);
          size_t replacement_offset = 0;
          size_t output_offset = 0;
          while (replacement_offset < REPLACEMENT_LENGTH &&
                 output_offset < LBUF_SIZE - 1) {
            const char CHARACTER = *(const char *)checked_storage_at_const(
                tar, REPLACEMENT_LENGTH + 1, sizeof(char), replacement_offset);
            if (CHARACTER == '%' &&
                replacement_offset + 1 < REPLACEMENT_LENGTH &&
                *(const char *)checked_storage_at_const(
                    tar, REPLACEMENT_LENGTH + 1, sizeof(char),
                    replacement_offset + 1) == '*') {
              *(char *)checked_storage_at(buff, LBUF_SIZE, sizeof(char),
                                          output_offset++) = '*';
              replacement_offset += 2;
            } else if (CHARACTER == '*') {
              char *destination = checked_storage_at(
                  buff, LBUF_SIZE, sizeof(char), output_offset);
              const size_t REMAINING = LBUF_SIZE - output_offset;
              strlcpy(destination, s, REMAINING);
              output_offset += strlen(destination);
              replacement_offset++;
            } else {
              *(char *)checked_storage_at(buff, LBUF_SIZE, sizeof(char),
                                          output_offset++) = CHARACTER;
              replacement_offset++;
            }
          }
          *(char *)checked_storage_at(buff, LBUF_SIZE, sizeof(char),
                                      output_offset) = 0;
          return buff;
        }
      }
    }
  }
  free_lbuf(buff);
  return nullptr;
}
