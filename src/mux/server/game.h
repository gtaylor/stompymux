/* game.h - Core notifications, database dumps, and shutdown interface. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/commands/command_invocation.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"

typedef struct EvaluationContext EvaluationContext;
typedef struct CommandContext CommandContext;
typedef struct CommandInvocation CommandInvocation;
typedef struct MuxServer MuxServer;
typedef struct ServerControl ServerControl;

/* Message forwarding directives. */
constexpr int MSG_INV = 2;           /* Forward message to contents. */
constexpr int MSG_INV_L = 4;         /* ... only if message passes @listen. */
constexpr int MSG_INV_EXITS = 8;     /* Forward through audible exits. */
constexpr int MSG_NBR = 16;          /* Forward message to neighbors. */
constexpr int MSG_NBR_A = 32;        /* ... only if I am audible. */
constexpr int MSG_NBR_EXITS = 64;    /* Also forward to neighbor exits. */
constexpr int MSG_NBR_EXITS_A = 128; /* ... only if I am audible. */
constexpr int MSG_LOC = 256;         /* Send to my location. */
constexpr int MSG_LOC_A = 512;       /* ... only if I am audible. */
/* 1024 is reserved for removed forwarding-list delivery. */
constexpr int MSG_ME = 2048;        /* Send to me. */
constexpr int MSG_S_INSIDE = 4096;  /* Originator is inside target. */
constexpr int MSG_S_OUTSIDE = 8192; /* Originator is outside target. */
constexpr int MSG_ME_ALL = MSG_ME | MSG_INV_EXITS;
constexpr int MSG_F_CONTENTS = MSG_INV;
constexpr int MSG_F_UP = MSG_NBR_A | MSG_LOC_A;
constexpr int MSG_F_DOWN = MSG_INV_L;

constexpr int DUMP_STRUCT = 1;   /* Dump flat structure file. */
constexpr int DUMP_TEXT = 2;     /* Dump text file. */
constexpr int DUMP_OPTIMIZE = 3; /* Reorganize the database file. */

constexpr int SHUTDN_NORMAL = 0;   /* Normal shutdown. */
constexpr int SHUTDN_PANIC = 1;    /* Write a panic dump file. */
constexpr int SHUTDN_EXIT = 2;     /* Exit from shutdown code. */
constexpr int SHUTDN_COREDUMP = 4; /* Produce a coredump. */
constexpr int SHUTDN_KILLED = 8;   /* Preserve a killed snapshot. */

void do_shutdown(CommandInvocation *invocation);
void server_shutdown(ServerControl *control, DbRef player, int key,
                     const char *message);
int dump_database_internal(ServerControl *control, int dump_type);
void dump_database(ServerControl *control);
void fork_and_dump(ServerControl *control, int key);
#ifdef notify_except
#pragma push_macro("notify_except")
#undef notify_except
#define RESTORE_NOTIFY_EXCEPT_MACRO
#endif
void notify_except(EvaluationContext *evaluation, DbRef location, DbRef player,
                   DbRef exception, const char *message);
#ifdef RESTORE_NOTIFY_EXCEPT_MACRO
#pragma pop_macro("notify_except")
#undef RESTORE_NOTIFY_EXCEPT_MACRO
#endif
void notify_except2(EvaluationContext *evaluation, DbRef location, DbRef player,
                    DbRef exception1, DbRef exception2, const char *message);
void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...)
    __attribute__((format(printf, 3, 4)));
void notify_checked(EvaluationContext *evaluation, DbRef target, DbRef sender,
                    const char *message, int key);
int is_hearer(EvaluationContext *evaluation, DbRef object);
void report(CommandContext *command);
