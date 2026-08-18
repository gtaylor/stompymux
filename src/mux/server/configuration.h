/** @file
 * Configuration file parsing and administrative updates.
 */
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

/** Creates server configuration. */

ServerConfiguration *server_configuration_create(void);
/** Destroys server configuration. @param[in,out] configuration Server
 * configuration. */

void server_configuration_destroy(ServerConfiguration *configuration);
/** Initializes configuration. @param[out] context Operation context. */

void configuration_initialize(ConfigurationContext *context);
/** Executes configuration log not found. @param[in,out] context Operation
 * context. @param[in] player Player object. @param[in] cmd Cmd. @param[in]
 * thingname Thingname. @param[in] thing Thing. */

void configuration_log_not_found(ConfigurationContext *context, DbRef player,
                                 const char *cmd, const char *thingname,
                                 const char *thing);
/** Executes configuration log syntax. @param[in,out] context Operation context.
 * @param[in] player Player object. @param[in] cmd Cmd. @param[in] template
 * Template. @param[in] argument Command argument. */

void configuration_log_syntax(ConfigurationContext *context, DbRef player,
                              const char *cmd, const char *template,
                              const char *argument);
/** Executes configuration list access. @param[in,out] evaluation Expression
 * evaluation context. @param[in] registry Registry to use. @param[in] player
 * Player object. */

void configuration_list_access(EvaluationContext *evaluation,
                               const ConfigurationRegistry *registry,
                               DbRef player);
/** Executes configuration read. @param[in,out] context Operation context.
 * @param[in] fn Fn. */

int configuration_read(ConfigurationContext *context, const char *fn);
/** Sets configuration. @param[in,out] context Operation context. @param[in] cp
 * Cp. @param[in] ap Ap. @param[in] player Player object. */

int configuration_set(ConfigurationContext *context, const char *cp,
                      const char *ap, DbRef player);
/** Executes configuration modify bits. @param[in] call Call. */

int configuration_modify_bits(const ConfigurationCall *call);
