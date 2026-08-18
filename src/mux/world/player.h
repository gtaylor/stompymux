/** @file
 * Player account, authentication, and name-cache interface.
 */
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

/** Executes record login. @param[in,out] evaluation Expression evaluation
 * context. @param[in] player Player object. @param[in] successful Successful.
 * @param[in] occurred_at Occurred at. @param[in] host Host. @param[in] username
 * Username. */

void record_login(EvaluationContext *evaluation, DbRef player, bool successful,
                  time_t occurred_at, const char *host, const char *username);
/** Reports whether check pass. @param[in] world World. @param[in] player Player
 * object. @param[in] password Password. */

bool check_pass(WorldContext *world, DbRef player, const char *password);
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

/** Executes connect player. @param[in] request Request. */

DbRef connect_player(const PlayerConnectionRequest *request);
/** Executes create player. @param[in] request Request. */

DbRef create_player(const PlayerCreationRequest *request);
/** Executes add player name. @param[in] world World. @param[in] player Player
 * object. @param[in] name Name to use. */

int add_player_name(WorldContext *world, DbRef player, const char *name);
/** Executes delete player name. @param[in] world World. @param[in] player
 * Player object. @param[in] name Name to use. */

bool delete_player_name(WorldContext *world, DbRef player, const char *name);
/** Looks up lookup player. @param[in] world World. @param[in] doer Doer.
 * @param[in] name Name to use. @param[in] check Check. */

DbRef lookup_player(WorldContext *world, DbRef doer, const char *name,
                    int check);
/** Executes load player names. @param[in,out] world World. */

void load_player_names(WorldContext *world);
/** Adds badname. @param[in,out] world World. @param[in,out] name Name to use.
 */

void badname_add(WorldContext *world, char *name);
/** Removes badname. @param[in,out] world World. @param[in,out] name Name to
 * use. */

void badname_remove(WorldContext *world, char *name);
/** Executes badname check. @param[in] world World. @param[in] name Name to use.
 */

bool badname_check(WorldContext *world, const char *name);
/** Executes badname list. @param[in,out] evaluation Expression evaluation
 * context. @param[in,out] world World. @param[in] player Player object.
 * @param[in] prefix Prefix. */

void badname_list(EvaluationContext *evaluation, WorldContext *world,
                  DbRef player, const char *prefix);
/** Handles the last command. @param[in,out] invocation Command invocation. */

void do_last(CommandInvocation *invocation);
