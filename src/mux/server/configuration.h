/* configuration.h - Configuration file parsing and administrative updates */

#pragma once

#include "mux/database/db.h"

typedef struct ConfigurationContext ConfigurationContext;
typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

#define CONF_FILE "stompymux.toml"

ServerConfiguration *server_configuration_create(void);
void server_configuration_destroy(ServerConfiguration *configuration);
void configuration_initialize(ConfigurationContext *context);
void configuration_log_not_found(ConfigurationContext *context, DbRef player,
                                 const char *cmd, const char *thing_name,
                                 const char *thing);
void configuration_log_syntax(ConfigurationContext *context, DbRef player,
                              const char *cmd, const char *template,
                              const char *argument);
void configuration_list_access(EvaluationContext *evaluation, DbRef player);
int configuration_read(ConfigurationContext *context, char *filename);
int configuration_set(ConfigurationContext *context, char *name, char *value,
                      DbRef player);
int configuration_modify_bits(int *value, char *string, long extra,
                              DbRef player, char *command,
                              ConfigurationContext *context);
