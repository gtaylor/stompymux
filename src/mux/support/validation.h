/* validation.h - Input validation for names, attributes, and passwords. */

#pragma once

#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

bool is_integer(char *string);
bool is_number(const char *string);
bool ok_name(const ServerConfiguration *configuration, const char *name);
bool ok_stored_player_name(const ServerConfiguration *configuration,
                           const char *name);
bool ok_player_name(const ServerConfiguration *configuration, const char *name);
bool ok_new_player_name(const ServerConfiguration *configuration,
                        const char *name);
bool ok_password(const ServerConfiguration *configuration,
                 const char *password);
