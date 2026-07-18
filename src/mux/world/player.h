/* player.h - Player account, authentication, and name-cache interface. */

#pragma once

#include "mux/database/db.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;
typedef struct WorldContext WorldContext;

void record_login(EvaluationContext *evaluation, DbRef player, int is_new,
                  char *host, char *username, char *ip_address);
int check_pass(WorldContext *world, DbRef player, const char *password);
DbRef connect_player(EvaluationContext *evaluation, WorldContext *world,
                     char *name, char *password, char *host, char *username);
DbRef create_player(EvaluationContext *evaluation, char *name, char *password,
                    DbRef creator, int key);
int add_player_name(WorldContext *world, DbRef player, char *name);
int delete_player_name(WorldContext *world, DbRef player, char *name);
DbRef lookup_player(WorldContext *world, DbRef player, char *name, int check);
void load_player_names(WorldContext *world);
void badname_add(WorldContext *world, char *name);
void badname_remove(WorldContext *world, char *name);
int badname_check(WorldContext *world, char *name);
void badname_list(EvaluationContext *evaluation, WorldContext *world,
                  DbRef player, const char *name);
void do_password(CommandInvocation *invocation);
void do_last(CommandInvocation *invocation);
