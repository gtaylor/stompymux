#include <assert.h>
#include <stdint.h>
#include <stdlib.h>

#include "mux/server/configuration_catalog.h"
#include "mux/server/configuration_registry.h"
#include "mux/support/checked_storage.h"

static int fixture_interpreter(const ConfigurationCall *call [[maybe_unused]]) {
  return 0;
}

static const ConfigurationEntry ENTRY_TEMPLATES[] = {
    {"first", fixture_interpreter, 1, 2, 3},
    {"second", fixture_interpreter, 4, 5, (intptr_t)6},
};

static const NameTable LIST_TEMPLATES[] = {
    {"commands", 3, 7, LIST_COMMANDS},
    {"options", 1, 8, LIST_OPTIONS},
};

static const ConfigurationCatalogPolicy POLICY = {
    .maximum_location = 10,
    .list_options_location = UINTPTR_MAX,
};

static bool fixture_install(ConfigurationRegistry *registry) {
  return configuration_catalog_install(
      registry, ENTRY_TEMPLATES,
      sizeof(ENTRY_TEMPLATES) / sizeof(*ENTRY_TEMPLATES), LIST_TEMPLATES,
      sizeof(LIST_TEMPLATES) / sizeof(*LIST_TEMPLATES), POLICY);
}

int main(void) {
  ConfigurationRegistry first = {0};
  ConfigurationRegistry second = {0};
  assert(fixture_install(&first));
  assert(fixture_install(&second));

  assert(first.entries != second.entries);
  assert(first.list_options != second.list_options);
  assert(configuration_registry_entry_count(&first) == 2);
  assert(configuration_registry_entry_count(nullptr) == 0);
  assert(configuration_registry_entry_at(nullptr, 0) == nullptr);
  assert(configuration_registry_entry_at_const(nullptr, 0) == nullptr);
  assert(configuration_registry_entry_at(&first, 2) == nullptr);
  assert(configuration_registry_entry_at_const(&first, 2) == nullptr);
  assert(configuration_registry_list_options(nullptr) == nullptr);
  assert(configuration_registry_list_options_const(nullptr) == nullptr);
  assert(configuration_registry_entry_at(&first, 0)->pname ==
         ENTRY_TEMPLATES[0].pname);
  assert(configuration_registry_entry_at_const(&first, 1)->extra == 6);
  assert(configuration_registry_list_options(&first)[0].name ==
         LIST_TEMPLATES[0].name);
  const NameTable *first_option = checked_storage_at_const(
      configuration_registry_list_options_const(&first), 3, sizeof(NameTable),
      1);
  const NameTable *first_sentinel =
      checked_storage_at_const(first.list_options, 3, sizeof(NameTable), 2);
  const NameTable *second_sentinel =
      checked_storage_at_const(second.list_options, 3, sizeof(NameTable), 2);
  assert(first_option->flag == LIST_OPTIONS);
  assert(first_sentinel->name == nullptr);
  assert(second_sentinel->name == nullptr);

  first.entries[0].flags = 99;
  first.list_options[0].perm = 77;
  assert(second.entries[0].flags == 1);
  assert(second.list_options[0].perm == 7);
  ConfigurationEntry *owned_entries = first.entries;
  NameTable *owned_options = first.list_options;
  assert(!fixture_install(&first));
  assert(first.entries == owned_entries);
  assert(first.list_options == owned_options);
  assert(first.entries[0].flags == 99);

  configuration_catalog_release(&second);
  configuration_catalog_release(&second);
  assert(first.entries[0].flags == 99);
  assert(first.list_options[0].perm == 77);
  configuration_catalog_release(&first);

  ConfigurationRegistry recreated = {0};
  assert(fixture_install(&recreated));
  assert(recreated.entries[0].flags == 1);
  assert(recreated.list_options[0].perm == 7);
  configuration_catalog_release(&recreated);

  ConfigurationRegistry partial = {
      .entries = calloc(1, sizeof(*partial.entries)), .entry_count = 1};
  assert(partial.entries != nullptr);
  configuration_catalog_release(&partial);
  assert(partial.entries == nullptr);
  assert(partial.entry_count == 0);

  ConfigurationRegistry invalid = {0};
  assert(!configuration_catalog_install(&invalid, nullptr, 0, LIST_TEMPLATES, 2,
                                        POLICY));
  assert(!configuration_catalog_install(&invalid, ENTRY_TEMPLATES, 2, nullptr,
                                        0, POLICY));

  const ConfigurationEntry duplicate_entries[] = {
      {"same", fixture_interpreter, 0, 0, 0},
      {"same", fixture_interpreter, 0, 0, 0},
  };
  const ConfigurationEntry empty_name[] = {{"", fixture_interpreter, 0, 0, 0}};
  const ConfigurationEntry missing_interpreter[] = {
      {"missing", nullptr, 0, 0, 0}};
  const ConfigurationEntry invalid_location[] = {
      {"location", fixture_interpreter, 0, 11, 0}};
  const NameTable duplicate_options[] = {{"same", 1, 0, 1}, {"same", 1, 0, 2}};
  const NameTable empty_option[] = {{"", 1, 0, 1}};
  const NameTable zero_minlen[] = {{"option", 0, 0, 1}};
  const NameTable long_minlen[] = {{"option", 7, 0, 1}};

  assert(!configuration_catalog_install(&invalid, duplicate_entries, 2,
                                        LIST_TEMPLATES, 2, POLICY));
  assert(!configuration_catalog_install(&invalid, empty_name, 1, LIST_TEMPLATES,
                                        2, POLICY));
  assert(!configuration_catalog_install(&invalid, missing_interpreter, 1,
                                        LIST_TEMPLATES, 2, POLICY));
  assert(!configuration_catalog_install(&invalid, invalid_location, 1,
                                        LIST_TEMPLATES, 2, POLICY));
  assert(!configuration_catalog_install(&invalid, ENTRY_TEMPLATES, 2,
                                        duplicate_options, 2, POLICY));
  assert(!configuration_catalog_install(&invalid, ENTRY_TEMPLATES, 2,
                                        empty_option, 1, POLICY));
  assert(!configuration_catalog_install(&invalid, ENTRY_TEMPLATES, 2,
                                        zero_minlen, 1, POLICY));
  assert(!configuration_catalog_install(&invalid, ENTRY_TEMPLATES, 2,
                                        long_minlen, 1, POLICY));
  assert(!configuration_catalog_install(
      &invalid, ENTRY_TEMPLATES, 2, LIST_TEMPLATES, 2,
      (ConfigurationCatalogPolicy){.maximum_location = 0,
                                   .list_options_location = UINTPTR_MAX}));
  assert(!configuration_catalog_install(&invalid, ENTRY_TEMPLATES, SIZE_MAX,
                                        LIST_TEMPLATES, 2, POLICY));
  assert(!configuration_catalog_install(&invalid, ENTRY_TEMPLATES, 2,
                                        LIST_TEMPLATES, SIZE_MAX, POLICY));
  assert(invalid.entries == nullptr);
  assert(invalid.list_options == nullptr);
  configuration_catalog_release(&invalid);
  configuration_catalog_release(nullptr);
  return 0;
}
