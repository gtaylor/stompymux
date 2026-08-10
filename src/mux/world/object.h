/* object.h - Low-level object creation and deletion operations. */

#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

typedef struct WorldContext WorldContext;

DbRef start_home(WorldContext *world);
DbRef default_home(WorldContext *world);
int can_set_home(EvaluationContext *evaluation, DbRef player, DbRef thing,
                 DbRef home);
DbRef new_home(EvaluationContext *evaluation, DbRef player);
typedef struct CloneHomeRequest {
  EvaluationContext *evaluation;
  DbRef player;
  DbRef source;
} CloneHomeRequest;

DbRef clone_home(const CloneHomeRequest *request);

DbRef create_obj(EvaluationContext *evaluation, DbRef player, int object_type,
                 const char *name);
typedef struct ObjectCreationIdentity {
  EvaluationContext *evaluation;
  DbRef object;
  int type;
} ObjectCreationIdentity;

void object_apply_default_lua_parent(const ObjectCreationIdentity *identity);

typedef struct ObjectDestructionRequest {
  EvaluationContext *evaluation;
  DbRef player;
  DbRef object;
} ObjectDestructionRequest;

void destroy_obj(const ObjectDestructionRequest *request);
void empty_obj(EvaluationContext *evaluation, DbRef object);
void destroy_exit(EvaluationContext *evaluation, DbRef exit);
void destroy_thing(EvaluationContext *evaluation, DbRef thing);
void destroy_player(EvaluationContext *evaluation, DbRef player);
