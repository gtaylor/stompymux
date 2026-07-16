/* configuration.h - Configuration file parsing and administrative updates */

#pragma once

#include "mux/database/db.h"

#define CONF_FILE "netmux.conf"

void configuration_initialize(void);
void configuration_log_not_found(DbRef player, const char *cmd,
                                 const char *thing_name, const char *thing);
void configuration_log_syntax(DbRef player, const char *cmd,
                              const char *template, const char *argument);
void configuration_list_access(DbRef player);
int configuration_read(char *filename);
int configuration_set(char *name, char *value, DbRef player);
int configuration_modify_bits(int *value, char *string, long extra,
                              DbRef player, char *command);
