/* command_queue.h - Command queue data structures and scheduling entry points.
 */

#pragma once

#include "mux/network/mux_event.h" // IWYU pragma: keep
#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h"

// IWYU pragma: no_include "btech_context.h"
// IWYU pragma: no_include "mux/commands/command_context.h"
// IWYU pragma: no_include "mux/commands/command_runtime.h"
// IWYU pragma: no_include "mux/server/log.h"
// IWYU pragma: no_include "mux/server/mux_server.h"
// IWYU pragma: no_include "mux/world/world_context.h"

/* BQUE - Command queue */

typedef struct bque BQUE;
typedef struct BtechContext BtechContext;
typedef struct CommandQueue CommandQueue;
typedef struct CommandRuntime CommandRuntime;
typedef struct CommandContext CommandContext;
typedef struct PlayerCache PlayerCache;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct ServerLog ServerLog;
typedef struct WorldContext WorldContext;
typedef struct MuxTimer MuxTimer;

typedef struct CommandQueueDependencies CommandQueueDependencies;
struct CommandQueueDependencies {
  /* Every member is borrowed from MuxServer. */
  CommandRuntime *command_runtime;
  BtechContext *btech;
  ServerLog *log;
  WorldContext *world;
  RuntimeClock *clock;
  PlayerCache *players;
  CommandContext *background_command;
};
struct bque {
  BQUE *next;

  DbRef player; /* player who will do command */
  DbRef cause;  /* player causing command (for %N) */
  int waittime; /* time to run command */
  int queuetime;
  char *text;          /* owned command storage */
  char *comm;          /* command */
  MuxTimer *timer;     /* timer for the wait queue */
  CommandQueue *queue; /* scheduler that owns this entry */
};

/* Per object run queues */
typedef struct objqe OBJQE;

struct objqe {
  DbRef obj;
  BQUE *cque;
  BQUE *ctail;
  BQUE *wait_que;    // commands waiting on this object
  BQUE *pending_que; // obj's commands that are waiting
  struct objqe *next;
  int queued;
};

CommandQueue *
command_queue_create(const CommandQueueDependencies *dependencies);
void command_queue_set_lifecycle(CommandQueue *queue,
                                 ServerLifecycle *lifecycle);
void command_queue_destroy(CommandQueue *queue);
int cque_init(CommandQueue *queue);
int halt_que(CommandQueue *queue, DbRef player, DbRef cause);
void wait_que(CommandQueue *queue, DbRef player, DbRef cause, int wait,
              char *command);
int que_next(CommandQueue *queue);
int do_top(CommandQueue *queue, int command_count);
void recover_queue_deposits(CommandQueue *queue);
