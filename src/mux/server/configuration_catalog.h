/** @file
 * Immutable configuration catalog installation helpers.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "mux/server/configuration_registry.h"

typedef struct ConfigurationCatalogPolicy {
  uintptr_t maximum_location;
  uintptr_t list_options_location;
} ConfigurationCatalogPolicy;

/** Executes configuration catalog install. @param[in,out] registry Registry to
 * use. @param[in] entry_templates Entry templates. @param[in] entry_count
 * Number of entry entries. @param[in] list_option_templates List option
 * templates. @param[in] list_option_count Number of list option entries.
 * @param[in] policy Policy. */

bool configuration_catalog_install(ConfigurationRegistry *registry,
                                   const ConfigurationEntry *entry_templates,
                                   size_t entry_count,
                                   const NameTable *list_option_templates,
                                   size_t list_option_count,
                                   ConfigurationCatalogPolicy policy);
/** Executes configuration catalog release. @param[in,out] registry Registry to
 * use. */

void configuration_catalog_release(ConfigurationRegistry *registry);
