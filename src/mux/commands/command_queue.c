/*
 * command_queue.c -- commands and functions for manipulating the command queue
 */

#include "mux/server/platform.h"

#include <signal.h>

#include "btech/btech_context.h"
#include "mux/commands/command.h"
#include "mux/commands/command_helpers.h"
#include "mux/commands/command_queue.h"
#include "mux/commands/command_runtime.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/event_timer.h"
#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/world/match.h"
#include "mux/world/player_cache.h"
#include "mux/world/world_context.h"

struct bque *alloc_qentry(const char *s) {
  return (struct bque *)malloc(sizeof(struct bque));
}

void free_qentry(struct bque *b) {
  if (b)
    free(b);
}

struct CommandQueue {
  CommandRuntime *command_runtime;
  BtechContext *btech;
  ServerLog *log;
  WorldContext *world;
  RuntimeClock *clock;
  PlayerCache *players;
  CommandContext *background_command;
  ServerLifecycle *lifecycle;
  RedBlackTree object_queues;
  OBJQE *head;
  OBJQE *tail;
  BQUE *wait;
};

static void cque_free_entry(BQUE *entry);

static void command_queue_free_entries(BQUE *entry) {
  while (entry != nullptr) {
    BQUE *next = entry->next;

    free(entry->text);
    cque_free_entry(entry);
    entry = next;
  }
}

static void command_queue_free_object(void *key, void *data, void *arg) {
  OBJQE *object_queue = data;

  command_queue_free_entries(object_queue->cque);
  free(object_queue);
}

CommandQueue *
command_queue_create(const CommandQueueDependencies *dependencies) {
  CommandQueue *queue = calloc(1, sizeof(*queue));

  if (queue == nullptr || dependencies == nullptr) {
    free(queue);
    return nullptr;
  }
  queue->command_runtime = dependencies->command_runtime;
  queue->btech = dependencies->btech;
  queue->log = dependencies->log;
  queue->world = dependencies->world;
  queue->clock = dependencies->clock;
  queue->players = dependencies->players;
  queue->background_command = dependencies->background_command;
  if (!cque_init(queue)) {
    free(queue);
    return nullptr;
  }
  return queue;
}

void command_queue_set_lifecycle(CommandQueue *queue,
                                 ServerLifecycle *lifecycle) {
  if (queue != nullptr)
    queue->lifecycle = lifecycle;
}

void command_queue_destroy(CommandQueue *queue) {
  if (queue == nullptr)
    return;
  command_queue_free_entries(queue->wait);
  if (queue->object_queues != nullptr)
    red_black_tree_release(queue->object_queues, command_queue_free_object,
                           nullptr);
  free(queue);
}

static void cque_free_entry(BQUE *entry) {
  if (entry->timer != nullptr)
    mux_timer_destroy(entry->timer);
  free_qentry(entry);
}

static int objqe_compare(DbRef left, DbRef right, void *arg) {
  return (right > left) - (right < left);
}

int cque_init(CommandQueue *queue) {
  queue->object_queues = red_black_tree_init(
      (int (*)(void *, void *, void *))(GenericFnPtr)objqe_compare, nullptr);
  return 1;
}

static OBJQE *cque_find(CommandQueue *queue, DbRef player) {
  OBJQE *tmp = nullptr;

  if (queue->object_queues == nullptr) {
    cque_init(queue);
  }

  tmp = red_black_tree_find(queue->object_queues, (void *)player);

  if (!tmp && is_good_obj(queue->world->database, player)) {
    tmp = malloc(sizeof(OBJQE));
    tmp->obj = player;
    tmp->cque = nullptr;
    tmp->ctail = nullptr;
    tmp->next = nullptr;
    tmp->queued = 0;
    tmp->wait_que = nullptr;
    tmp->pending_que = nullptr;
    red_black_tree_insert(queue->object_queues, (void *)player, tmp);
  }

  return tmp;
}

static BQUE *cque_deque(CommandQueue *queue, DbRef player) {
  OBJQE *tmp;
  BQUE *cmd;

  tmp = cque_find(queue, player);
  if (!tmp)
    return nullptr;

  dassert(tmp);

  if (!tmp->cque)
    return nullptr;

  cmd = tmp->cque;
  if (!cmd->next) {
    tmp->cque = tmp->ctail = nullptr;
  } else {
    tmp->cque = cmd->next;
  }
  return cmd;
}

static void cque_enqueue(CommandQueue *queue, DbRef player, BQUE *cmd) {
  BQUE *point, *trail;
  OBJQE *tmp;

  cmd->next = nullptr;

  if (cmd->waittime <= queue->clock->now) {
    cmd->waittime = 0;
    tmp = cque_find(queue, player);

    dassert(tmp);

    if (!tmp->ctail) {
      tmp->cque = tmp->ctail = cmd;
      cmd->next = nullptr;
    } else {
      tmp->ctail->next = cmd;
      tmp->ctail = cmd;
      tmp->ctail->next = nullptr;
    }

    if (!tmp->queued) {
      if (!queue->head) {
        queue->head = queue->tail = tmp;
        tmp->next = nullptr;
      } else {
        queue->tail->next = tmp;
        queue->tail = tmp;
        queue->tail->next = nullptr;
      }
      tmp->queued = 1;
    }
  } else {
    mux_timer_start(cmd->timer,
                    (uint64_t)(cmd->waittime - queue->clock->now) * 1000, 0);
    for (point = queue->wait, trail = nullptr;
         point && point->waittime <= cmd->waittime; point = point->next) {
      trail = point;
    }
    cmd->next = point;
    if (trail != nullptr)
      trail->next = cmd;
    else
      queue->wait = cmd;
  }
}

static void wakeup_wait_que(MuxTimer *timer, void *arg) {
  BQUE *pending = (BQUE *)arg;
  CommandQueue *queue = pending->queue;
  BQUE *point;

  if (queue->wait == pending) {
    queue->wait = pending->next;
  } else {
    for (point = queue->wait; point; point = point->next) {
      if (point->next == pending) {
        point->next = point->next->next;
        break;
      }
    }
  }

  pending->waittime = 0;
  cque_enqueue(queue, pending->player, pending);
}

/*
 * ---------------------------------------------------------------------------
 * * que_want: Do we want this queue entry?
 */

static int que_want(GameDatabase *database, BQUE *entry, DbRef ptarg,
                    DbRef otarg) {
  if ((ptarg != NOTHING) && (ptarg != entry->player))
    return 0;
  if ((otarg != NOTHING) && (otarg != entry->player))
    return 0;
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * halt_que: Remove all queued commands from a certain player
 */

int halt_que(CommandQueue *queue, DbRef player, DbRef object) {
  BQUE *trail, *point, *next;
  OBJQE *pque;

  int numhalted;

  numhalted = 0;

  /* Player's que */
  // XXX: nuke queu

  pque = cque_find(queue, player);
  if (pque && pque->cque) {
    while ((point = cque_deque(queue, player)) != nullptr) {
      free(point->text);
      point->text = nullptr;
      cque_free_entry(point);
      point = nullptr;
      numhalted++;
    }
  }
  pque = cque_find(queue, object);
  if (pque && pque->cque) {
    while ((point = cque_deque(queue, object)) != nullptr) {
      free(point->text);
      point->text = nullptr;
      cque_free_entry(point);
      point = nullptr;
      numhalted++;
    }
  }

  /*
   * Wait queue
   */

  for (point = queue->wait, trail = nullptr; point; point = next)
    if (que_want(queue->world->database, point, player, object)) {
      numhalted++;
      if (trail)
        trail->next = next = point->next;
      else
        queue->wait = next = point->next;
      mux_timer_stop(point->timer);
      free(point->text);
      cque_free_entry(point);
    } else
      next = (trail = point)->next;

  if (player == NOTHING)
    player = object;
  if (object == NOTHING)
    queue_set(queue->players, player, 0);
  else
    queue_adjust(queue->players, player, -numhalted);
  return numhalted;
}

/*
 * ---------------------------------------------------------------------------
 * * do_halt: Command interface to halt_que.
 */

void do_halt(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *target = invocation->first;
  CommandQueue *queue = invocation->context->runtime->commands;
  DbRef player_targ, obj_targ;
  int numhalted;

  if ((key & HALT_ALL) && !is_wizard(queue->world->database, player)) {
    notify(evaluation, player, "Permission denied.");
    return;
  }
  /*
   * Figure out what to halt
   */

  if (!target || !*target) {
    obj_targ = NOTHING;
    if (key & HALT_ALL) {
      player_targ = NOTHING;
    } else {
      player_targ = player;
    }
  } else {
    if (is_wizard(queue->world->database, player))
      obj_targ = match_thing(&invocation->context->match, player, target);
    else
      obj_targ = match_controlled(&invocation->context->match, player, target);

    if (obj_targ == NOTHING)
      return;
    if (key & HALT_ALL) {
      notify(evaluation, player, "Can't specify a target and /all");
      return;
    }
    if (typeof_obj(queue->world->database, obj_targ) == TYPE_PLAYER) {
      player_targ = obj_targ;
      obj_targ = NOTHING;
    } else {
      player_targ = NOTHING;
    }
  }

  numhalted = halt_que(queue, player_targ, obj_targ);
  if (is_quiet(queue->world->database, player))
    return;
  if (numhalted == 1)
    notify(evaluation, player, "1 queue entries removed.");
  else
    notify_printf(evaluation, player, "%d queue entries removed.", numhalted);
}

/*
 * ---------------------------------------------------------------------------
 * * setup_que: Set up a queue entry.
 */

static BQUE *setup_que(CommandQueue *queue, DbRef player, DbRef cause,
                       char *command) {
  EvaluationContext *evaluation = &queue->background_command->evaluation;
  int maximum;
  BQUE *tmp;

  /*
   * Can we run commands at all?
   */

  if (is_halted(queue->world->database, player))
    return nullptr;

  /*
   * Wizards and their objs may queue up to db_top+1 cmds. Players are
   * * * * * * * limited to QUEUE_QUOTA. -mnp
   */

  maximum = queue_maximum(queue->players, player);
  if (queue_adjust(queue->players, player, 1) > maximum) {
    notify(evaluation, player,
           "Run away objects: too many commands queued.  Halted.");
    halt_que(queue, player, NOTHING);

    /*
     * halt also means no command execution allowed
     */
    s_halted(queue->world->database, player);
    return nullptr;
  }
  /*
   * We passed all the tests
   */

  /*
   * Calculate the length of the save string
   */

  /*
   * Create the qeue entry and load the save string
   */

  tmp = malloc(sizeof(BQUE));
  memset(tmp, 0, sizeof(BQUE));
  tmp->text = strdup(command ? command : "");
  tmp->comm = tmp->text;
  /*
   * Load the rest of the queue block
   */

  tmp->timer = mux_timer_create(server_lifecycle_loop(queue->lifecycle),
                                wakeup_wait_que, tmp);
  if (tmp->timer == nullptr) {
    free(tmp->text);
    free_qentry(tmp);
    return nullptr;
  }

  tmp->player = player;
  tmp->waittime = 0;
  tmp->next = nullptr;
  tmp->cause = cause;
  tmp->queue = queue;
  return tmp;
}

/*
 * ---------------------------------------------------------------------------
 * * wait_que: Add commands to the timed wait queue.
 */

void wait_que(CommandQueue *queue, DbRef player, DbRef cause, int wait,
              char *command) {
  BQUE *cmd;
  if (queue->world->configuration->is_command_queue_enabled)
    cmd = setup_que(queue, player, cause, command);
  else
    cmd = nullptr;

  if (cmd == nullptr) {
    return;
  }

  if (wait > 0) {
    cmd->waittime = (int)(queue->clock->now + wait);
  } else {
    cmd->waittime = 0;
  }

  cque_enqueue(queue, player, cmd);
}

/*
 * ---------------------------------------------------------------------------
 * * do_wait: Command interface to wait_que
 */

void do_wait(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  char *event = invocation->first;
  char *cmd = invocation->second;
  CommandQueue *queue = invocation->context->runtime->commands;

  if (!is_number(event)) {
    notify(evaluation, player, "Wait time must be a number.");
    return;
  }

  wait_que(queue, player, cause, atoi(event), cmd);
}

/*
 * ---------------------------------------------------------------------------
 * * do_top: Execute the command at the top of the queue
 */

int do_top(CommandQueue *queue, int ncmds) {
  BQUE *tmp;
  DbRef object;
  int count;
  char *command, *cp;

  if (!queue->head)
    return 0;

  count = 0;

  while (count < ncmds && queue->head) {
    if (!queue->head)
      break;

    object = queue->head->obj;
    tmp = cque_deque(queue, object);

    if (!queue->head->cque) {
      queue->head->queued = 0;
      queue->head = queue->head->next;
      if (queue->head == nullptr)
        queue->tail = nullptr;
    } else {
      queue->tail->next = queue->head;
      queue->tail = queue->tail->next;
      queue->head = queue->head->next;
      queue->tail->next = nullptr;
    }
    if (!tmp)
      continue;

    dassert(tmp);
    count++;
    if ((object >= 0) && !is_going(queue->world->database, object)) {
      CommandContext context;
      BtechCommandScope btech_scope;

      if (!command_context_initialize(&context, queue->command_runtime,
                                      queue->btech, queue->log, object,
                                      tmp->cause, nullptr, false)) {
        free(tmp->text);
        cque_free_entry(tmp);
        continue;
      }
      context.debug_command = "< do_top >";
      btech_command_scope_enter(&btech_scope, context.btech, &context);
      queue_adjust(queue->players, object, -1);
      if (!is_halted(queue->world->database, object)) {
        command = tmp->comm;

        if (command) {
          while (command) {
            cp = parse_to(queue->world->configuration, &command, ';', 0);
            if (cp && *cp) {
              process_command(&context, cp, nullptr, 0);
            }
          }
        }
      }
      btech_command_scope_leave(&btech_scope);
      command_context_destroy(&context);
    }
    free(tmp->text);
    cque_free_entry(tmp);
  }

  return count;
}
