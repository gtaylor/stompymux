/* Private configuration registry and value interpreter interfaces. */

#pragma once

#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h"
#include "mux/support/name_table.h"

typedef int (*ConfigurationInterpreter)(void *value, char *text, long extra,
                                        DbRef player, char *command,
                                        ConfigurationContext *context);

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

int configuration_status_from_succfail(DbRef player, char *command, int success,
                                       int failure,
                                       ConfigurationContext *context);

int cf_int(int *value, char *text, long extra, DbRef player, char *command,
           ConfigurationContext *context);
int cf_bool(int *value, char *text, long extra, DbRef player, char *command,
            ConfigurationContext *context);
int cf_bool_bit(int *value, char *text, long extra, DbRef player, char *command,
                ConfigurationContext *context);
int cf_string(int *value, char *text, long extra, DbRef player, char *command,
              ConfigurationContext *context);
int cf_flagalias(int *value, char *text, long extra, DbRef player,
                 char *command, ConfigurationContext *context);
int cf_set_flags(void *value, char *text, long extra, DbRef player,
                 char *command, ConfigurationContext *context);
int cf_badname(int *value, char *text, long extra, DbRef player, char *command,
               ConfigurationContext *context);
int cf_site(long **value, char *text, long extra, DbRef player, char *command,
            ConfigurationContext *context);
int cf_named_color(void *value, char *text, long extra, DbRef player,
                   char *command, ConfigurationContext *context);
int cf_osc8_preset(void *value, char *text, long extra, DbRef player,
                   char *command, ConfigurationContext *context);
int cf_cf_access(int *value, char *text, long extra, DbRef player,
                 char *command, ConfigurationContext *context);
