/* Server-owned mutable configuration catalog state. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "mux/server/configuration_interpreter.h"
#include "mux/support/name_table.h"

typedef struct ConfigurationEntry ConfigurationEntry;
struct ConfigurationEntry {
  const char *pname;
  ConfigurationInterpreter interpreter;
  int flags;
  uintptr_t location;
  intptr_t extra;
};

typedef ConfigurationEntry CONF;

typedef struct ConfigurationRegistry ConfigurationRegistry;
struct ConfigurationRegistry {
  ConfigurationEntry *entries;
  size_t entry_count;
  NameTable *list_options;
  size_t list_option_count;
};

typedef enum ConfigurationListOption {
  LIST_COMMANDS = 2,
  LIST_FLAGS = 4,
  LIST_GLOBALS = 6,
  LIST_LOGGING = 8,
  LIST_DF_FLAGS = 9,
  LIST_PERMS = 10,
  LIST_OPTIONS = 12,
  LIST_CONF_PERMS = 15,
  LIST_SITEINFO = 16,
  LIST_POWERS = 17,
  LIST_SWITCHES = 18,
  LIST_PROCESS = 21,
  LIST_BADNAMES = 22,
  LIST_LOGFILES = 23,
} ConfigurationListOption;

bool configuration_registry_initialize(ConfigurationRegistry *registry);
void configuration_registry_destroy(ConfigurationRegistry *registry);
size_t
configuration_registry_entry_count(const ConfigurationRegistry *registry);
ConfigurationEntry *
configuration_registry_entry_at(ConfigurationRegistry *registry, size_t index);
const ConfigurationEntry *
configuration_registry_entry_at_const(const ConfigurationRegistry *registry,
                                      size_t index);
NameTable *configuration_registry_list_options(ConfigurationRegistry *registry);
const NameTable *configuration_registry_list_options_const(
    const ConfigurationRegistry *registry);
