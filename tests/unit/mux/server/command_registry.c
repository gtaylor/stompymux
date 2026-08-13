#include <assert.h>

#include "mux/commands/command_catalog.h"
#include "mux/commands/command_internal.h"
#include "mux/server/server_registries.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"

static const NameTable SHARED_SWITCHES[] = {{"quiet", 1, 4, SW_MULTIPLE | 8},
                                            {nullptr, 0, 0, 0}};
static const CommandDefinition DEFINITIONS[] = {
    {"goto", SHARED_SWITCHES, 1, 2, 3, {nullptr}},
    {"other", SHARED_SWITCHES, 5, 6, 7, {nullptr}},
    {"\"", nullptr, 0, 0, 0, {nullptr}},
    {":", nullptr, 0, 0, 0, {nullptr}},
    {";", nullptr, 0, 0, 0, {nullptr}},
    {"\\", nullptr, 0, 0, 0, {nullptr}},
    {"#", nullptr, 0, 0, 0, {nullptr}},
};

static void macro_handler(MatchContext *match, MacroRegistry *registry,
                          DbRef player, char *arguments) {
  (void)match;
  (void)registry;
  (void)player;
  (void)arguments;
}

static const MACENT MACROS[] = {{"one", macro_handler}, {"two", macro_handler}};

int main(void) {
  CommandRegistry first = {0};
  CommandRegistry second = {0};
  assert(command_catalog_install(&first, DEFINITIONS,
                                 sizeof(DEFINITIONS) / sizeof(*DEFINITIONS)));
  assert(command_catalog_install(&second, DEFINITIONS,
                                 sizeof(DEFINITIONS) / sizeof(*DEFINITIONS)));

  CMDENT *first_goto = hash_table_find("goto", &first.commands);
  CMDENT *first_other = hash_table_find("other", &first.commands);
  CMDENT *second_goto = hash_table_find("goto", &second.commands);
  assert(first_goto != second_goto);
  assert(first_goto->switches != second_goto->switches);
  assert(first_goto->switches == first_other->switches);
  const NameTable *sentinel = checked_storage_at_const(
      first_goto->switches, 2, sizeof(*first_goto->switches), 1);
  assert(sentinel->name == nullptr);
  first_goto->perms = 99;
  first_goto->switches[0].perm = 77;
  assert(second_goto->perms == 1);
  assert(second_goto->switches[0].perm == 4);
  assert(first.goto_command == first_goto);
  assert(second.goto_command == second_goto);
  assert(first.prefix_commands['"'] != second.prefix_commands['"']);
  for (size_t i = 0; i < 256; i++) {
    CMDENT *prefix = *(CMDENT *const *)checked_storage_at_const(
        first.prefix_commands, 256, sizeof(*first.prefix_commands), i);
    bool supported = i == '"' || i == ':' || i == ';' || i == '\\' || i == '#';
    assert((prefix != nullptr) == supported);
  }

  assert(command_registry_add_alias(&first, "go", first_goto));
  assert(hash_table_find("go", &first.commands) == first_goto);
  assert(command_registry_add_switch_alias(&first, "gq", first_goto,
                                           &first_goto->switches[0]));
  CMDENT *switch_alias = hash_table_find("gq", &first.commands);
  assert(switch_alias != first_goto);
  assert(switch_alias->switches == first_goto->switches);
  assert(switch_alias->perms == (99 | 77));
  assert(second.switch_alias_count == 0);

  command_catalog_release(&second);
  assert(hash_table_find("gq", &first.commands) == switch_alias);
  command_catalog_release(&first);
  command_catalog_release(&first);
  assert(command_registry_builtin_count(&first) == 0);

  CommandRegistry missing = {0};
  assert(!command_catalog_install(&missing, &DEFINITIONS[1], 1));
  assert(command_registry_builtin_count(&missing) == 1);
  command_catalog_release(&missing);

  const CommandDefinition DUPLICATES[] = {
      {"goto", nullptr, 0, 0, 0, {nullptr}},
      {"goto", nullptr, 0, 0, 0, {nullptr}},
  };
  CommandRegistry duplicate = {0};
  assert(!command_catalog_install(&duplicate, DUPLICATES,
                                  sizeof(DUPLICATES) / sizeof(*DUPLICATES)));
  assert(command_registry_builtin_count(&duplicate) == 1);
  command_catalog_release(&duplicate);

  CommandRegistry occupied = {0};
  assert(command_catalog_install(&occupied, DEFINITIONS,
                                 sizeof(DEFINITIONS) / sizeof(*DEFINITIONS)));
  CMDENT *owned_goto = occupied.goto_command;
  assert(!command_catalog_install(&occupied, DEFINITIONS,
                                  sizeof(DEFINITIONS) / sizeof(*DEFINITIONS)));
  assert(occupied.goto_command == owned_goto);
  command_catalog_release(&occupied);

  CommandRegistry dirty_prefix = {0};
  CMDENT marker = {0};
  dirty_prefix.prefix_commands[1] = &marker;
  assert(!command_catalog_install(&dirty_prefix, DEFINITIONS,
                                  sizeof(DEFINITIONS) / sizeof(*DEFINITIONS)));
  command_catalog_release(&dirty_prefix);

  CommandRegistry macros = {0};
  assert(
      macro_catalog_install(&macros, MACROS, sizeof(MACROS) / sizeof(*MACROS)));
  assert(hash_table_find_const("one", &macros.macros) == MACROS);
  assert(hash_table_find_const("missing", &macros.macros) == nullptr);
  assert(!macro_catalog_install(&macros, MACROS,
                                sizeof(MACROS) / sizeof(*MACROS)));
  command_catalog_release(&macros);
  command_catalog_release(&macros);

  const MACENT DUPLICATE_MACROS[] = {{"same", macro_handler},
                                     {"same", macro_handler}};
  CommandRegistry duplicate_macros = {0};
  assert(!macro_catalog_install(&duplicate_macros, DUPLICATE_MACROS,
                                sizeof(DUPLICATE_MACROS) /
                                    sizeof(*DUPLICATE_MACROS)));
  command_catalog_release(&duplicate_macros);

  CommandRegistry recreated = {0};
  assert(command_catalog_install(&recreated, DEFINITIONS,
                                 sizeof(DEFINITIONS) / sizeof(*DEFINITIONS)));
  assert(recreated.goto_command->perms == 1);
  const NameTable *default_switch =
      checked_storage_at_const(recreated.goto_command->switches, 2,
                               sizeof(*recreated.goto_command->switches), 0);
  assert(default_switch->perm == 4);
  command_catalog_release(&recreated);
  return 0;
}
