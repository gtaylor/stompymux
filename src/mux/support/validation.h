/** @file
 * Input validation for names, attributes, and passwords.
 */
#pragma once

#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

/** Reports whether is integer. @param[in] string String to process. */

bool is_integer(char *string);
/** Reports whether is number. @param[in] string String to process. */

bool is_number(const char *string);
/** Executes ok name. @param[in] configuration Server configuration. @param[in]
 * name Name to use. */

bool ok_name(const ServerConfiguration *configuration, const char *name);
/** Executes ok stored player name. @param[in] configuration Server
 * configuration. @param[in] name Name to use. */

bool ok_stored_player_name(const ServerConfiguration *configuration,
                           const char *name);
/** Executes ok player name. @param[in] configuration Server configuration.
 * @param[in] name Name to use. */

bool ok_player_name(const ServerConfiguration *configuration, const char *name);
/** Executes ok new player name. @param[in] configuration Server configuration.
 * @param[in] name Name to use. */

bool ok_new_player_name(const ServerConfiguration *configuration,
                        const char *name);
/** Executes ok password. @param[in] configuration Server configuration.
 * @param[in] password Password. */

bool ok_password(const ServerConfiguration *configuration,
                 const char *password);
