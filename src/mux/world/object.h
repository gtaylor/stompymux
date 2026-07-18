/* object.h - Low-level object creation, deletion, and parent operations. */

#pragma once

#include "mux/database/db.h"

typedef struct WorldContext WorldContext;

DbRef start_home(WorldContext *world);
DbRef default_home(WorldContext *world);
int can_set_home(EvaluationContext *evaluation, DbRef player, DbRef thing,
                 DbRef home);
DbRef new_home(EvaluationContext *evaluation, DbRef player);
DbRef clone_home(EvaluationContext *evaluation, DbRef player, DbRef thing);

DbRef create_obj(EvaluationContext *evaluation, DbRef player, int object_type,
                 char *name);
void destroy_obj(EvaluationContext *evaluation, DbRef player, DbRef object);
void database_check(EvaluationContext *evaluation, DbRef player, int key);
void divest_object(EvaluationContext *evaluation, DbRef object);
void empty_obj(EvaluationContext *evaluation, DbRef object);
void destroy_exit(EvaluationContext *evaluation, DbRef exit);
void destroy_thing(EvaluationContext *evaluation, DbRef thing);
void destroy_player(EvaluationContext *evaluation, DbRef player);
