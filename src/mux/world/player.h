/* player.h - Player account, authentication, and name-cache interface. */

#pragma once

#include <stdbool.h>
#include <time.h>

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;
typedef struct WorldContext WorldContext;

void record_login(EvaluationContext *evaluation, DbRef player, bool successful,
                  time_t occurred_at, const char *host, const char *username);
int check_pass(WorldContext *world, DbRef player, const char *password);
DbRef connect_player(EvaluationContext *evaluation, WorldContext *world,
                     char *name, char *password, char *host, char *username);
DbRef create_player(EvaluationContext *evaluation, char *name, char *password);
int add_player_name(WorldContext *world, DbRef player, char *name);
int delete_player_name(WorldContext *world, DbRef player, char *name);
DbRef lookup_player(WorldContext *world, DbRef player, char *name, int check);
void load_player_names(WorldContext *world);
void badname_add(WorldContext *world, char *name);
void badname_remove(WorldContext *world, char *name);
int badname_check(WorldContext *world, char *name);
void badname_list(EvaluationContext *evaluation, WorldContext *world,
                  DbRef player, const char *name);
void do_last(CommandInvocation *invocation);
