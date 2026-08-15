/* configuration.h - Configuration file parsing and administrative updates */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/server/configuration_context.h"
#include "mux/server/configuration_interpreter.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct ConfigurationContext ConfigurationContext;
typedef struct ConfigurationRegistry ConfigurationRegistry;
typedef struct EvaluationContext EvaluationContext;
typedef struct ServerConfiguration ServerConfiguration;

#define CONF_FILE "stompymux.toml"

ServerConfiguration *server_configuration_create(void);
void server_configuration_destroy(ServerConfiguration *configuration);
void configuration_initialize(ConfigurationContext *context);
void configuration_log_not_found(ConfigurationContext *context, DbRef player,
                                 const char *cmd, const char *thingname,
                                 const char *thing);
void configuration_log_syntax(ConfigurationContext *context, DbRef player,
                              const char *cmd, const char *template,
                              const char *argument);
void configuration_list_access(EvaluationContext *evaluation,
                               const ConfigurationRegistry *registry,
                               DbRef player);
int configuration_read(ConfigurationContext *context, const char *fn);
int configuration_set(ConfigurationContext *context, const char *cp,
                      const char *ap, DbRef player);
int configuration_modify_bits(const ConfigurationCall *call);
