/* Private configuration registry and value interpreter interfaces. */

#pragma once

#include <stdint.h>

#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h"
#include "mux/server/configuration_interpreter.h"

constexpr uintptr_t CONFIGURATION_LIST_NAMES_LOCATION = UINTPTR_MAX;

typedef struct ConfigurationParseCounts {
  int success;
  int failure;
} ConfigurationParseCounts;

int configuration_status_from_counts(const ConfigurationCall *call,
                                     ConfigurationParseCounts counts);

int cf_int(const ConfigurationCall *call);
int cf_player_name_length_limit(const ConfigurationCall *call);
int cf_techtime_multiplier(const ConfigurationCall *call);
int cf_bool(const ConfigurationCall *call);
int cf_bool_bit(const ConfigurationCall *call);
int cf_string(const ConfigurationCall *call);
int cf_flagalias(const ConfigurationCall *call);
int cf_set_flags(const ConfigurationCall *call);
int cf_badname(const ConfigurationCall *call);
int cf_site(const ConfigurationCall *call);
int cf_named_color(const ConfigurationCall *call);
int cf_osc8_preset(const ConfigurationCall *call);
int cf_bootstrap_objects_clear(const ConfigurationCall *call);
int cf_bootstrap_object(const ConfigurationCall *call);
int cf_cf_access(const ConfigurationCall *call);
int cf_ntab_access(const ConfigurationCall *call);
