/** @file
 * Low-level object creation and deletion operations.
 */
#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

typedef struct WorldContext WorldContext;

/** Executes start home. @param[in,out] world World. */

DbRef start_home(WorldContext *world);
/** Executes default home. @param[in,out] world World. */

DbRef default_home(WorldContext *world);
/** Reports whether can set home. @param[in] evaluation Expression evaluation
 * context. @param[in] player Player object. @param[in] thing Thing. @param[in]
 * home Home. */

bool can_set_home(EvaluationContext *evaluation, DbRef player, DbRef thing,
                  DbRef home);
/** Executes new home. @param[in,out] evaluation Expression evaluation context.
 * @param[in] player Player object. */

DbRef new_home(EvaluationContext *evaluation, DbRef player);
typedef struct CloneHomeRequest {
  EvaluationContext *evaluation;
  DbRef player;
  DbRef source;
} CloneHomeRequest;

/** Executes clone home. @param[in] request Request. */

DbRef clone_home(const CloneHomeRequest *request);

/** Executes create obj. @param[in] evaluation Expression evaluation context.
 * @param[in] player Player object. @param[in] objtype Objtype. @param[in] name
 * Name to use. */

DbRef create_obj(EvaluationContext *evaluation, DbRef player, int objtype,
                 const char *name);
typedef struct ObjectCreationIdentity {
  EvaluationContext *evaluation;
  DbRef object;
  int type;
} ObjectCreationIdentity;

/** Executes object apply default lua parent. @param[in] identity Identity. */

void object_apply_default_lua_parent(const ObjectCreationIdentity *identity);

typedef struct ObjectDestructionRequest {
  EvaluationContext *evaluation;
  DbRef player;
  DbRef object;
} ObjectDestructionRequest;

/** Executes destroy obj. @param[in] request Request. */

void destroy_obj(const ObjectDestructionRequest *request);
/** Executes empty obj. @param[in,out] evaluation Expression evaluation context.
 * @param[in] object Game object. */

void empty_obj(EvaluationContext *evaluation, DbRef object);
/** Executes destroy exit. @param[in,out] evaluation Expression evaluation
 * context. @param[in] exit Exit. */

void destroy_exit(EvaluationContext *evaluation, DbRef exit);
/** Executes destroy thing. @param[in,out] evaluation Expression evaluation
 * context. @param[in] thing Thing. */

void destroy_thing(EvaluationContext *evaluation, DbRef thing);
/** Executes destroy player. @param[in,out] evaluation Expression evaluation
 * context. @param[in] victim Victim. */

void destroy_player(EvaluationContext *evaluation, DbRef victim);
