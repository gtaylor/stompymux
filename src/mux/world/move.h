/* move.h - Object movement and enter-command helper interface. */

#pragma once

#include "mux/objects/db.h"

typedef struct CommandInvocation CommandInvocation;
typedef struct EvaluationContext EvaluationContext;

void move_object(EvaluationContext *evaluation, DbRef thing, DbRef destination);
void move_via_generic(EvaluationContext *evaluation, DbRef thing,
                      DbRef destination, DbRef cause, int hush);
void move_via_exit(EvaluationContext *evaluation, DbRef thing,
                   DbRef destination, DbRef cause, DbRef exit, int hush);
int move_via_teleport(EvaluationContext *evaluation, DbRef thing,
                      DbRef destination, DbRef cause, int hush);
void move_exit(EvaluationContext *evaluation, DbRef player, DbRef exit,
               const char *name, int quiet);
void do_enter_internal(EvaluationContext *evaluation, DbRef player,
                       DbRef target, int key);
void do_move(CommandInvocation *invocation);
void move_command(EvaluationContext *evaluation, DbRef player, DbRef cause,
                  int key, char *direction);
