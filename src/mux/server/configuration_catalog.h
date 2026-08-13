/* Immutable configuration catalog installation helpers. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "mux/server/configuration_registry.h"

typedef struct ConfigurationCatalogPolicy {
  uintptr_t maximum_location;
  uintptr_t list_options_location;
} ConfigurationCatalogPolicy;

bool configuration_catalog_install(ConfigurationRegistry *registry,
                                   const ConfigurationEntry *entry_templates,
                                   size_t entry_count,
                                   const NameTable *list_option_templates,
                                   size_t list_option_count,
                                   ConfigurationCatalogPolicy policy);
void configuration_catalog_release(ConfigurationRegistry *registry);
