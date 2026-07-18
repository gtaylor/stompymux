/* configuration.h - Configuration file parsing and administrative updates */

#pragma once

#include "mux/database/db.h"

typedef struct MuxServer MuxServer;

#define CONF_FILE "stompymux.toml"

void configuration_initialize(MuxServer *server);
void configuration_log_not_found(MuxServer *server, DbRef player,
                                 const char *cmd, const char *thing_name,
                                 const char *thing);
void configuration_log_syntax(MuxServer *server, DbRef player, const char *cmd,
                              const char *template, const char *argument);
void configuration_list_access(MuxServer *server, DbRef player);
int configuration_read(MuxServer *server, char *filename);
int configuration_set(MuxServer *server, char *name, char *value, DbRef player);
int configuration_modify_bits(int *value, char *string, long extra,
                              DbRef player, char *command, MuxServer *server);
