/* command_queue.h - Command queue data structures and scheduling entry points.
 */

#pragma once
#include "mux/server/event_timer.h"
#include "mux/server/platform.h"

/* BQUE - Command queue */

typedef struct bque BQUE;
struct bque {
  BQUE *next;

  DbRef player; /* player who will do command */
  DbRef cause;  /* player causing command (for %N) */
  DbRef sem;    /* blocking semaphore */
  int waittime; /* time to run command */
  int queuetime;
  int attr;                /* blocking attribute */
  char *text;              /* buffer for comm, env, and scr text */
  char *comm;              /* command */
  char *env[NUM_ENV_VARS]; /* environment vars */
  char *scr[NUM_ENV_VARS]; /* temp vars */
  int nargs;               /* How many args I have */
  MuxTimer *timer;         /* timer for the wait queue */
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

int cque_init(void);
void do_second(void);
int nfy_que(DbRef player, int key, int wait, int attr);
int halt_que(DbRef player, DbRef cause);
void wait_que(DbRef player, DbRef cause, int wait, DbRef sem, int attr,
              char *command, char *arguments[], int argument_count,
              char *commands[]);
int que_next(void);
int do_top(int command_count);
void recover_queue_deposits(void);
