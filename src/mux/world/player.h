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
typedef struct PlayerConnectionRequest {
  EvaluationContext *evaluation;
  WorldContext *world;
  const char *name;
  const char *password;
  const char *host;
  const char *username;
} PlayerConnectionRequest;

typedef struct PlayerCreationRequest {
  EvaluationContext *evaluation;
  const char *name;
  const char *password;
} PlayerCreationRequest;

DbRef connect_player(const PlayerConnectionRequest *request);
DbRef create_player(const PlayerCreationRequest *request);
int add_player_name(WorldContext *world, DbRef player, const char *name);
int delete_player_name(WorldContext *world, DbRef player, const char *name);
DbRef lookup_player(WorldContext *world, DbRef doer, const char *name,
                    int check);
void load_player_names(WorldContext *world);
void badname_add(WorldContext *world, char *name);
void badname_remove(WorldContext *world, char *name);
int badname_check(WorldContext *world, char *name);
void badname_list(EvaluationContext *evaluation, WorldContext *world,
                  DbRef player, const char *prefix);
void do_last(CommandInvocation *invocation);
