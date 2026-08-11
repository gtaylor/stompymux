/* Private configuration registry and value interpreter interfaces. */

#pragma once

#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h"
#include "mux/server/configuration_interpreter.h"
#include "mux/support/name_table.h"

typedef struct ConfigurationEntry ConfigurationEntry;
struct ConfigurationEntry {
  const char *pname;
  ConfigurationInterpreter interpreter;
  int flags;
  uintptr_t location;
  long extra;
};

typedef ConfigurationEntry CONF;
constexpr uintptr_t CONFIGURATION_LIST_NAMES_LOCATION = UINTPTR_MAX;
extern CONF conftable[];
size_t configuration_entry_count(void);
CONF *configuration_entry_at(size_t index);
extern NameTable logdata_nametab[];
extern NameTable logoptions_nametab[];
extern NameTable access_nametab[];
extern NameTable list_names[];

typedef struct ConfigurationParseCounts {
  int success;
  int failure;
} ConfigurationParseCounts;

int configuration_status_from_counts(const ConfigurationCall *call,
                                     ConfigurationParseCounts counts);

int cf_int(const ConfigurationCall *call);
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
