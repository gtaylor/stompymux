/* validation.h - Input validation for names, attributes, and passwords. */

#pragma once

#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

int is_integer(char *string);
int is_number(const char *string);
int ok_name(const ServerConfiguration *configuration, const char *name);
int ok_player_name(const ServerConfiguration *configuration, const char *name);
int ok_new_player_name(const ServerConfiguration *configuration,
                       const char *name);
int ok_password(const ServerConfiguration *configuration, const char *password);
