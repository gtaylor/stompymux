/* game.h - Core notifications, database dumps, and shutdown interface. */

#pragma once

#include "mux/database/db.h"

typedef struct EvaluationContext EvaluationContext;
typedef struct CommandContext CommandContext;
typedef struct CommandInvocation CommandInvocation;
typedef struct MuxServer MuxServer;
typedef struct ServerControl ServerControl;

void do_shutdown(CommandInvocation *invocation);
void server_shutdown(ServerControl *control, DbRef player, int key,
                     const char *message);
int dump_database_internal(ServerControl *control, int dump_type);
void dump_database(ServerControl *control);
void fork_and_dump(ServerControl *control, int key);
void notify_except(EvaluationContext *evaluation, DbRef location, DbRef player,
                   DbRef exception, const char *message);
void notify_except2(EvaluationContext *evaluation, DbRef location, DbRef player,
                    DbRef exception1, DbRef exception2, const char *message);
void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...)
    __attribute__((format(printf, 3, 4)));
int check_filter(EvaluationContext *evaluation, DbRef object, DbRef player,
                 int filter, const char *message);
void notify_checked(EvaluationContext *evaluation, DbRef target, DbRef sender,
                    const char *message, int key);
int is_hearer(EvaluationContext *evaluation, DbRef object);
void report(CommandContext *command);
int attribute_match(EvaluationContext *evaluation, DbRef thing, DbRef player,
                    char type, const char *string, int check_parent);
int list_check(EvaluationContext *evaluation, DbRef player, DbRef thing,
               char type, char *string, int check_parent);
