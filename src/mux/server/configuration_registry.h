/** @file
 * Server-owned mutable configuration catalog state.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "mux/server/configuration_interpreter.h"
#include "mux/support/name_table.h"

typedef struct ConfigurationEntry ConfigurationEntry;
typedef struct ServerConfiguration ServerConfiguration;

typedef enum ConfigurationValueKind : int {
  CONFIGURATION_VALUE_UNSUPPORTED,
  CONFIGURATION_VALUE_INTEGER,
  CONFIGURATION_VALUE_NUMBER,
  CONFIGURATION_VALUE_BOOLEAN,
  CONFIGURATION_VALUE_INTEGER_BOOLEAN,
  CONFIGURATION_VALUE_STRING,
  CONFIGURATION_VALUE_LUA_ERROR_REPORTING,
} ConfigurationValueKind;

typedef struct ConfigurationValue ConfigurationValue;
struct ConfigurationValue {
  ConfigurationValueKind kind;
  union {
    int integer;
    double number;
    bool boolean;
    const char *string;
  } as;
};

typedef enum ConfigurationQueryStatus : int {
  CONFIGURATION_QUERY_OK,
  CONFIGURATION_QUERY_NOT_FOUND,
  CONFIGURATION_QUERY_UNSUPPORTED,
} ConfigurationQueryStatus;

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
  ConfigurationValueKind *value_kinds;
  size_t entry_count;
  NameTable *list_options;
  size_t list_option_count;
};

typedef enum ConfigurationListOption : int {
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

/** Initializes configuration registry. @param[out] registry Registry to use. */

bool configuration_registry_initialize(ConfigurationRegistry *registry);
/** Destroys configuration registry. @param[in,out] registry Registry to use. */

void configuration_registry_destroy(ConfigurationRegistry *registry);
/** Counts configuration registry entry. @param[in] registry Registry to use. */

size_t
configuration_registry_entry_count(const ConfigurationRegistry *registry);
/** Returns configuration registry entry at. @param[in] registry Registry to
 * use. @param[in] index Zero-based index. */

ConfigurationEntry *
configuration_registry_entry_at(ConfigurationRegistry *registry, size_t index);
/** Returns configuration registry entry at. @param[in] registry Registry to
 * use. @param[in] index Zero-based index. */

const ConfigurationEntry *
configuration_registry_entry_at_const(const ConfigurationRegistry *registry,
                                      size_t index);
/** Executes configuration registry list options. @param[in,out] registry
 * Registry to use. */

NameTable *configuration_registry_list_options(ConfigurationRegistry *registry);
/** Executes configuration registry list options const. @param[in] registry
 * Registry to use. */

const NameTable *configuration_registry_list_options_const(
    const ConfigurationRegistry *registry);

/** Queries a scalar configuration value by its exact directive name.
 * @param[in] registry Registry to search. @param[in] configuration Current
 * server configuration. @param[in] name Exact, case-sensitive directive name.
 * @param[out] value Typed borrowed result on success. @return Query status. */

ConfigurationQueryStatus
configuration_registry_query(const ConfigurationRegistry *registry,
                             const ServerConfiguration *configuration,
                             const char *name, ConfigurationValue *value);
