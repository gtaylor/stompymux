/* Registry-owned copies of immutable configuration catalog templates. */

#include "mux/server/configuration_catalog.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "mux/server/configuration_registry.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/name_table.h"

static bool
configuration_catalog_policy_is_valid(ConfigurationCatalogPolicy policy) {
  return (policy.maximum_location > 0 &&
          policy.list_options_location > policy.maximum_location) != 0;
}

static bool configuration_entries_are_valid(const ConfigurationEntry *templates,
                                            size_t count,
                                            ConfigurationCatalogPolicy policy) {
  if (count > SIZE_MAX / sizeof(*templates))
    return false;
  for (size_t index = 0; index < count; index++) {
    const ConfigurationEntry *entry =
        checked_storage_at_const(templates, count, sizeof(*templates), index);
    if (entry->pname == nullptr || entry->pname[0] == '\0' ||
        entry->interpreter == nullptr ||
        (entry->location > policy.maximum_location &&
         entry->location != policy.list_options_location))
      return false;
    for (size_t previous = 0; previous < index; previous++) {
      const ConfigurationEntry *earlier = checked_storage_at_const(
          templates, count, sizeof(*templates), previous);
      if (strcmp(entry->pname, earlier->pname) == 0)
        return false;
    }
  }
  return true;
}

static bool list_options_are_valid(const NameTable *templates, size_t count) {
  if (count == SIZE_MAX || count > SIZE_MAX / sizeof(*templates) - 1)
    return false;
  for (size_t index = 0; index < count; index++) {
    const NameTable *option =
        checked_storage_at_const(templates, count, sizeof(*templates), index);
    if (option->name == nullptr || option->name[0] == '\0' ||
        option->minlen <= 0 || (size_t)option->minlen > strlen(option->name))
      return false;
    for (size_t previous = 0; previous < index; previous++) {
      const NameTable *earlier = checked_storage_at_const(
          templates, count, sizeof(*templates), previous);
      if (strcmp(option->name, earlier->name) == 0)
        return false;
    }
  }
  return true;
}

bool configuration_catalog_install(ConfigurationRegistry *registry,
                                   const ConfigurationEntry *entry_templates,
                                   size_t entry_count,
                                   const NameTable *list_option_templates,
                                   size_t list_option_count,
                                   ConfigurationCatalogPolicy policy) {
  if (registry == nullptr || entry_templates == nullptr || entry_count == 0 ||
      list_option_templates == nullptr || list_option_count == 0 ||
      registry->entries != nullptr || registry->value_kinds != nullptr ||
      registry->entry_count != 0 || registry->list_options != nullptr ||
      registry->list_option_count != 0 ||
      !configuration_catalog_policy_is_valid(policy) ||
      !configuration_entries_are_valid(entry_templates, entry_count, policy) ||
      !list_options_are_valid(list_option_templates, list_option_count))
    return false;

  ConfigurationEntry *entries =
      checked_storage_try_allocate_array(entry_count, sizeof(*entries));
  ConfigurationValueKind *value_kinds =
      checked_storage_try_allocate_array(entry_count, sizeof(*value_kinds));
  NameTable *list_options = checked_storage_try_allocate_array(
      list_option_count + 1, sizeof(*list_options));
  if (entries == nullptr || value_kinds == nullptr || list_options == nullptr) {
    free(list_options);
    free(value_kinds);
    free(entries);
    return false;
  }

  memcpy(entries, entry_templates, entry_count * sizeof(*entries));
  memcpy(list_options, list_option_templates,
         list_option_count * sizeof(*list_options));
  registry->entries = entries;
  registry->value_kinds = value_kinds;
  registry->entry_count = entry_count;
  registry->list_options = list_options;
  registry->list_option_count = list_option_count;
  return true;
}

void configuration_catalog_release(ConfigurationRegistry *registry) {
  if (registry == nullptr)
    return;
  free(registry->list_options);
  free(registry->value_kinds);
  free(registry->entries);
  memset(registry, 0, sizeof(*registry));
}

void configuration_registry_destroy(ConfigurationRegistry *registry) {
  configuration_catalog_release(registry);
}

ConfigurationQueryStatus
configuration_registry_query(const ConfigurationRegistry *registry,
                             const ServerConfiguration *configuration,
                             const char *name, ConfigurationValue *value) {
  if (registry == nullptr || configuration == nullptr || name == nullptr ||
      value == nullptr)
    return CONFIGURATION_QUERY_NOT_FOUND;

  for (size_t index = 0; index < registry->entry_count; index++) {
    const ConfigurationEntry *entry =
        configuration_registry_entry_at_const(registry, index);
    if (strcmp(entry->pname, name) != 0)
      continue;
    const ConfigurationValueKind KIND =
        *(const ConfigurationValueKind *)checked_storage_at_const(
            registry->value_kinds, registry->entry_count,
            sizeof(*registry->value_kinds), index);
    if (KIND == CONFIGURATION_VALUE_UNSUPPORTED || entry->location == 0 ||
        entry->location > sizeof(*configuration))
      return CONFIGURATION_QUERY_UNSUPPORTED;

    const void *storage = checked_storage_region_const(
        configuration, sizeof(*configuration), (size_t)entry->location - 1, 1);
    *value = (ConfigurationValue){.kind = KIND};
    switch (KIND) {
    case CONFIGURATION_VALUE_INTEGER:
      value->as.integer = *(const int *)storage;
      break;
    case CONFIGURATION_VALUE_NUMBER:
      value->as.number = *(const double *)storage;
      break;
    case CONFIGURATION_VALUE_BOOLEAN:
      value->as.boolean = *(const bool *)storage;
      break;
    case CONFIGURATION_VALUE_INTEGER_BOOLEAN:
      value->as.boolean = *(const int *)storage != 0;
      value->kind = CONFIGURATION_VALUE_BOOLEAN;
      break;
    case CONFIGURATION_VALUE_STRING:
      value->as.string = storage;
      break;
    case CONFIGURATION_VALUE_LUA_ERROR_REPORTING: {
      const LuaErrorReporting REPORTING = *(const LuaErrorReporting *)storage;
      switch (REPORTING) {
      case LUA_ERROR_REPORTING_OFF:
        value->as.string = "off";
        break;
      case LUA_ERROR_REPORTING_WIZARDS:
        value->as.string = "wizards";
        break;
      case LUA_ERROR_REPORTING_ALL:
        value->as.string = "all";
        break;
      }
      value->kind = CONFIGURATION_VALUE_STRING;
      break;
    }
    case CONFIGURATION_VALUE_UNSUPPORTED:
      return CONFIGURATION_QUERY_UNSUPPORTED;
    }
    return CONFIGURATION_QUERY_OK;
  }
  return CONFIGURATION_QUERY_NOT_FOUND;
}

size_t
configuration_registry_entry_count(const ConfigurationRegistry *registry) {
  return registry == nullptr ? 0 : registry->entry_count;
}

ConfigurationEntry *
configuration_registry_entry_at(ConfigurationRegistry *registry, size_t index) {
  if (registry == nullptr || index >= registry->entry_count)
    return nullptr;
  return checked_storage_at(registry->entries, registry->entry_count,
                            sizeof(*registry->entries), index);
}

const ConfigurationEntry *
configuration_registry_entry_at_const(const ConfigurationRegistry *registry,
                                      size_t index) {
  if (registry == nullptr || index >= registry->entry_count)
    return nullptr;
  return checked_storage_at_const(registry->entries, registry->entry_count,
                                  sizeof(*registry->entries), index);
}

NameTable *
configuration_registry_list_options(ConfigurationRegistry *registry) {
  return registry == nullptr ? nullptr : registry->list_options;
}

const NameTable *configuration_registry_list_options_const(
    const ConfigurationRegistry *registry) {
  return registry == nullptr ? nullptr : registry->list_options;
}
