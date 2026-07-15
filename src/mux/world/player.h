/* player.h - Player account, authentication, and name-cache interface. */

#pragma once

#include "mux/database/db.h"

void record_login(DbRef player, int is_new, char *host, char *username,
                  char *ip_address);
int check_pass(DbRef player, const char *password);
DbRef connect_player(char *name, char *password, char *host, char *username);
DbRef create_player(char *name, char *password, DbRef creator, int key);
int add_player_name(DbRef player, char *name);
int delete_player_name(DbRef player, char *name);
DbRef lookup_player(DbRef player, char *name, int check);
void load_player_names(void);
void badname_add(char *name);
void badname_remove(char *name);
int badname_check(char *name);
void badname_list(DbRef player, const char *name);
