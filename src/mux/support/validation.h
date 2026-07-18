/* validation.h - Input validation for names, attributes, and passwords. */

#pragma once

typedef struct ServerConfiguration ServerConfiguration;

int is_integer(char *string);
int is_number(char *string);
int ok_name(const ServerConfiguration *configuration, const char *name);
int ok_player_name(const ServerConfiguration *configuration, const char *name);
int ok_new_player_name(const ServerConfiguration *configuration,
                       const char *name);
int ok_attr_name(const char *attribute_name);
int ok_password(const ServerConfiguration *configuration, const char *password);
