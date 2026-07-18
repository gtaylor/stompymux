/*
 * command_queue.c -- commands and functions for manipulating the command queue
 */

#include "mux/server/platform.h"

#include <signal.h>

#include "mux/commands/command.h"
#include "mux/commands/command_queue.h"
#include "mux/commands/functions.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/event_timer.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/world/match.h"
#include "mux/world/player_cache.h"

struct bque *alloc_qentry(const char *s) {
  return (struct bque *)malloc(sizeof(struct bque));
}

void free_qentry(struct bque *b) {
  if (b)
    free(b);
}

struct CommandQueue {
  MuxServer *server;
  RedBlackTree object_queues;
  OBJQE *head;
  OBJQE *tail;
  BQUE *wait;
  BQUE *semaphore_first;
  BQUE *semaphore_last;
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

CommandQueue *command_queue_create(MuxServer *server) {
  CommandQueue *queue = calloc(1, sizeof(*queue));

  if (queue == nullptr)
    return nullptr;
  queue->server = server;
  if (!cque_init(queue)) {
    free(queue);
    return nullptr;
  }
  return queue;
}

void command_queue_destroy(CommandQueue *queue) {
  if (queue == nullptr)
    return;
  command_queue_free_entries(queue->wait);
  command_queue_free_entries(queue->semaphore_first);
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

  if (!tmp && is_good_obj(&queue->server->database, player)) {
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

  if (cmd->sem == NOTHING) {
    /*
     * No semaphore, put on wait queue if wait value specified.
     * Otherwise put on the normal queue.
     */

    if (cmd->waittime <= queue->server->clock.now) {
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
      mux_timer_start(
          cmd->timer,
          (uint64_t)(cmd->waittime - queue->server->clock.now) * 1000, 0);
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
  } else {
    cmd->next = nullptr;
    if (queue->semaphore_last != nullptr)
      queue->semaphore_last->next = cmd;
    else
      queue->semaphore_first = cmd;
    queue->semaphore_last = cmd;
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
 * * add_to: Adjust an object's queue or semaphore count.
 */

static int add_to(GameDatabase *database, DbRef player, int am, int attrnum) {
  int num;
  long aflags;
  DbRef aowner;
  char buff[20] = {0};
  char *atr_gotten;

  num = atoi(atr_gotten =
                 attribute_get(database, player, attrnum, &aowner, &aflags));
  free_lbuf(atr_gotten);
  num += am;
  if (num)
    snprintf(buff, sizeof(buff), "%d", num);
  else
    *buff = '\0';
  attribute_add_raw(database, player, attrnum, buff);
  return (num);
}

/*
 * ---------------------------------------------------------------------------
 * * que_want: Do we want this queue entry?
 */

static int que_want(GameDatabase *database, BQUE *entry, DbRef ptarg,
                    DbRef otarg) {
  if ((ptarg != NOTHING) &&
      (ptarg != game_object_owner(database, entry->player)))
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
    if (que_want(&queue->server->database, point, player, object)) {
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

  /*
   * Semaphore queue
   */

  for (point = queue->semaphore_first, trail = nullptr; point; point = next)
    if (que_want(&queue->server->database, point, player, object)) {
      numhalted++;
      if (trail)
        trail->next = next = point->next;
      else
        queue->semaphore_first = next = point->next;
      if (point == queue->semaphore_last)
        queue->semaphore_last = trail;
      add_to(&queue->server->database, point->sem, -1, point->attr);
      free(point->text);
      cque_free_entry(point);
    } else
      next = (trail = point)->next;

  if (player == NOTHING)
    player = game_object_owner(&queue->server->database, object);
  if (object == NOTHING)
    queue_set(queue->server->players, player, 0);
  else
    queue_adjust(queue->server->players, player, -numhalted);
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
  CommandQueue *queue = invocation->context->server->commands;
  DbRef player_targ, obj_targ;
  int numhalted;

  if ((key & HALT_ALL) && !is_wizard(&queue->server->database, player)) {
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
      player_targ = game_object_owner(&queue->server->database, player);
      if (typeof_obj(&queue->server->database, player) != TYPE_PLAYER)
        obj_targ = player;
    }
  } else {
    if (is_wizard(&queue->server->database, player))
      obj_targ = match_thing(&invocation->context->match, player, target);
    else
      obj_targ = match_controlled(&invocation->context->match, player, target);

    if (obj_targ == NOTHING)
      return;
    if (key & HALT_ALL) {
      notify(evaluation, player, "Can't specify a target and /all");
      return;
    }
    if (typeof_obj(&queue->server->database, obj_targ) == TYPE_PLAYER) {
      player_targ = obj_targ;
      obj_targ = NOTHING;
    } else {
      player_targ = NOTHING;
    }
  }

  numhalted = halt_que(queue, player_targ, obj_targ);
  if (is_quiet(&queue->server->database, player))
    return;
  if (numhalted == 1)
    notify(evaluation, game_object_owner(&queue->server->database, player),
           "1 queue entries removed.");
  else
    notify_printf(evaluation,
                  game_object_owner(&queue->server->database, player),
                  "%d queue entries removed.", numhalted);
}

/*
 * ---------------------------------------------------------------------------
 * * nfy_que: Notify commands from the queue and perform or discard them.
 */

int nfy_que(CommandQueue *queue, DbRef sem, int attr, int key, int count) {
  BQUE *point, *trail, *next;
  int num;
  long aflags;
  DbRef aowner;
  char *str;

  if (attr) {
    str = attribute_get(&queue->server->database, sem, attr, &aowner, &aflags);
    num = atoi(str);
    free_lbuf(str);
  } else {
    num = 1;
  }

  if (num > 0) {
    num = 0;
    for (point = queue->semaphore_first, trail = nullptr; point; point = next) {
      if ((point->sem == sem) && ((point->attr == attr) || !attr)) {
        num++;
        if (trail)
          trail->next = next = point->next;
        else
          queue->semaphore_first = next = point->next;
        if (point == queue->semaphore_last)
          queue->semaphore_last = trail;

        /*
         * Either run or discard the command
         */

        if (key != NFY_DRAIN) {
          point->sem = NOTHING;
          point->waittime = 0;
          cque_enqueue(queue, point->player, point);
        } else {
          queue_adjust(
              queue->server->players,
              game_object_owner(&queue->server->database, point->player), -1);
          free(point->text);
          cque_free_entry(point);
        }
      } else {
        next = (trail = point)->next;
      }

      /*
       * If we've notified enough, exit
       */

      if ((key == NFY_NFY) && (num >= count))
        next = nullptr;
    }
  } else {
    num = 0;
  }

  /*
   * Update the sem waiters count
   */

  if (key == NFY_NFY)
    add_to(&queue->server->database, sem, -count, attr);
  else
    attribute_clear(&queue->server->database, sem, attr);

  return num;
}

/*
 * ---------------------------------------------------------------------------
 * * do_notify: Command interface to nfy_que
 */

void do_notify(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *what = invocation->first;
  char *count = invocation->second;
  CommandQueue *queue = invocation->context->server->commands;
  DbRef thing, aowner;
  int loccount, attr = -1;
  long aflags;
  Attribute *ap;
  char *obj;

  obj = parse_to(queue->server->configuration, &what, '/', 0);
  init_match(&invocation->context->match, player, obj, NOTYPE);
  match_everything(&invocation->context->match, 0);

  if ((thing = noisy_match_result(&invocation->context->match)) < 0) {
    notify(evaluation, player, "No match.");
  } else if (!is_controls(evaluation, player, thing)) {
    notify(evaluation, player, "Permission denied.");
  } else {
    if (!what || !*what) {
      ap = nullptr;
    } else {
      ap = attribute_by_name(invocation->context->world->database, what);
    }

    if (!ap) {
      attr = A_SEMAPHORE;
    } else {
      /* Do they have permission to set this attribute? */
      attribute_parent_get_info(&queue->server->database, thing, ap->number,
                                &aowner, &aflags);
      if (set_attr(evaluation, player, thing, ap, aflags)) {
        attr = ap->number;
      } else {
        notify_quiet(evaluation, player, "Permission denied.");
        return;
      }
    }

    if (count && *count)
      loccount = atoi(count);
    else
      loccount = 1;
    if (loccount > 0) {
      nfy_que(queue, thing, attr, key, loccount);
      if (!(is_quiet(&queue->server->database, player) ||
            is_quiet(&queue->server->database, thing))) {
        if (key == NFY_DRAIN)
          notify_quiet(evaluation, player, "Drained.");
        else
          notify_quiet(evaluation, player, "Notified.");
      }
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * setup_que: Set up a queue entry.
 */

static BQUE *setup_que(CommandQueue *queue, DbRef player, DbRef cause,
                       char *command, char *args[], int nargs, char *sargs[]) {
  EvaluationContext *evaluation = &queue->server->background_command.evaluation;
  int a;
  size_t tlen;
  BQUE *tmp;
  char *tptr;

  /*
   * Can we run commands at all?
   */

  if (is_halted(&queue->server->database, player))
    return nullptr;

  /*
   * Wizards and their objs may queue up to db_top+1 cmds. Players are
   * * * * * * * limited to QUEUE_QUOTA. -mnp
   */

  a = queue_maximum(queue->server->players,
                    game_object_owner(&queue->server->database, player));
  if (queue_adjust(queue->server->players,
                   game_object_owner(&queue->server->database, player),
                   1) > a) {
    notify(evaluation, game_object_owner(&queue->server->database, player),
           "Run away objects: too many commands queued.  Halted.");
    halt_que(queue, game_object_owner(&queue->server->database, player),
             NOTHING);

    /*
     * halt also means no command execution allowed
     */
    s_halted(&queue->server->database, player);
    return nullptr;
  }
  /*
   * We passed all the tests
   */

  /*
   * Calculate the length of the save string
   */

  tlen = 0;
  if (command)
    tlen = strlen(command) + 1;
  if (nargs > NUM_ENV_VARS)
    nargs = NUM_ENV_VARS;
  for (a = 0; a < nargs; a++) {
    if (args[a])
      tlen += (strlen(args[a]) + 1);
  }
  if (sargs) {
    for (a = 0; a < NUM_ENV_VARS; a++) {
      if (sargs[a])
        tlen += (strlen(sargs[a]) + 1);
    }
  }
  /*
   * Create the qeue entry and load the save string
   */

  tmp = malloc(sizeof(BQUE));
  memset(tmp, 0, sizeof(BQUE));
  tmp->comm = nullptr;
  for (a = 0; a < NUM_ENV_VARS; a++) {
    tmp->env[a] = nullptr;
  }
  for (a = 0; a < MAX_GLOBAL_REGS; a++) {
    tmp->scr[a] = nullptr;
  }

  tptr = tmp->text = malloc(tlen);
  if (command) {
    StringCopy(tptr, command);
    tmp->comm = tptr;
    tptr += (strlen(command) + 1);
  }
  for (a = 0; a < nargs; a++) {
    if (args[a]) {
      StringCopy(tptr, args[a]);
      tmp->env[a] = tptr;
      tptr += (strlen(args[a]) + 1);
    }
  }
  if (sargs) {
    for (a = 0; a < MAX_GLOBAL_REGS; a++) {
      if (sargs[a]) {
        StringCopy(tptr, sargs[a]);
        tmp->scr[a] = tptr;
        tptr += (strlen(sargs[a]) + 1);
      }
    }
  }
  /*
   * Load the rest of the queue block
   */

  tmp->timer = mux_timer_create(server_lifecycle_loop(queue->server->lifecycle),
                                wakeup_wait_que, tmp);
  if (tmp->timer == nullptr) {
    free(tmp->text);
    free_qentry(tmp);
    return nullptr;
  }

  tmp->player = player;
  tmp->waittime = 0;
  tmp->next = nullptr;
  tmp->sem = NOTHING;
  tmp->attr = 0;
  tmp->cause = cause;
  tmp->nargs = nargs;
  tmp->queue = queue;
  return tmp;
}

/*
 * ---------------------------------------------------------------------------
 * * wait_que: Add commands to the wait or semaphore queues.
 */

void wait_que(CommandQueue *queue, DbRef player, DbRef cause, int wait,
              DbRef sem, int attr, char *command, char *args[], int nargs,
              char *sargs[]) {
  BQUE *cmd;
  if (queue->server->configuration->is_interpreter_enabled)
    cmd = setup_que(queue, player, cause, command, args, nargs, sargs);
  else
    cmd = nullptr;

  if (cmd == nullptr) {
    return;
  }

  if (wait > 0) {
    cmd->waittime = (int)(queue->server->clock.now + wait);
  } else {
    cmd->waittime = 0;
  }

  cmd->sem = sem;
  cmd->attr = attr;

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
  char **cargs = invocation->vector;
  int ncargs = invocation->vector_count;
  CommandQueue *queue = invocation->context->server->commands;
  DbRef thing, aowner;
  int howlong, num, attr;
  long aflags;
  char *what;
  Attribute *ap;

  /*
   * If arg1 is all numeric, do simple (non-sem) timed wait.
   */

  if (is_number(event)) {
    howlong = atoi(event);
    wait_que(queue, player, cause, howlong, NOTHING, 0, cmd, cargs, ncargs,
             invocation->context->evaluation.registers);
    return;
  }
  /*
   * Semaphore wait with optional timeout
   */

  what = parse_to(queue->server->configuration, &event, '/', 0);
  init_match(&invocation->context->match, player, what, NOTYPE);
  match_everything(&invocation->context->match, 0);

  thing = noisy_match_result(&invocation->context->match);
  if (!is_good_obj(&queue->server->database, thing)) {
    notify(evaluation, player, "No match.");
  } else if (!is_controls(evaluation, player, thing)) {
    notify(evaluation, player, "Permission denied.");
  } else {

    /*
     * Get timeout, default 0
     */

    if (event && *event && is_number(event)) {
      attr = A_SEMAPHORE;
      howlong = atoi(event);
    } else {
      attr = A_SEMAPHORE;
      howlong = 0;
    }

    if (event && *event && !is_number(event)) {
      ap = attribute_by_name(invocation->context->world->database, event);
      if (!ap) {
        attr = mkattr(invocation->context->world->database, event);
        if (attr <= 0) {
          notify_quiet(evaluation, player, "Invalid attribute.");
          return;
        }
        ap = attribute_by_number(invocation->context->world->database, attr);
      }
      attribute_parent_get_info(&queue->server->database, thing, ap->number,
                                &aowner, &aflags);
      if (attr && set_attr(evaluation, player, thing, ap, aflags)) {
        attr = ap->number;
        howlong = 0;
      } else {
        notify_quiet(evaluation, player, "Permission denied.");
        return;
      }
    }

    num = add_to(&queue->server->database, thing, 1, attr);
    if (num <= 0) {

      /*
       * thing over-notified, run the command immediately
       */

      thing = NOTHING;
      howlong = 0;
    }
    wait_que(queue, player, cause, howlong, thing, attr, cmd, cargs, ncargs,
             invocation->context->evaluation.registers);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_second: Check the wait and semaphore queues for commands to remove.
 */

void do_second(CommandQueue *queue) {
  BQUE *trail, *point, *next;
  const char *cmdsave;

  /*
   * move contents of low priority queue onto end of normal one this
   * helps to keep objects from getting out of control since
   * its affects on other objects happen only after one
   * second  this should allow @halt to be type before
   * getting blown away  by scrolling text
   */

  if (!queue->server->configuration->is_dequeue_enabled)
    return;

  cmdsave = queue->server->background_command.debug_command;
  queue->server->background_command.debug_command = "< do_second >";

  /*
   * Note: the point->waittime test would be 0 except the command is
   * being put in the low priority queue to be done in one
   * second anyways
   */

  /*
   * Check the semaphore queue for expired timed-waits
   */

  for (point = queue->semaphore_first, trail = nullptr; point; point = next) {
    if (point->waittime == 0) {
      next = (trail = point)->next;
      continue; /*
                 * Skip if not timed-wait
                 */
    }
    if (point->waittime <= queue->server->clock.now) {
      if (trail != nullptr)
        trail->next = next = point->next;
      else
        queue->semaphore_first = next = point->next;
      if (point == queue->semaphore_last)
        queue->semaphore_last = trail;
      add_to(&queue->server->database, point->sem, -1, point->attr);
      point->sem = NOTHING;
      point->waittime = 0;
      printk("promoting, %ld/%s", point->player, point->comm);
      cque_enqueue(queue, point->player, point);
    } else
      next = (trail = point)->next;
  }
  queue->server->background_command.debug_command = cmdsave;
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * do_top: Execute the command at the top of the queue
 */

int do_top(CommandQueue *queue, int ncmds) {
  BQUE *tmp;
  CommandContext *previous_context = queue->server->btech.command_context;
  DbRef object;
  int count, i;
  char *command, *cp;

  if (!queue->server->configuration->is_dequeue_enabled)
    return 0;

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
    if ((object >= 0) && !is_going(&queue->server->database, object)) {
      CommandContext context;

      if (!command_context_initialize(&context, queue->server, object,
                                      tmp->cause, nullptr, false)) {
        free(tmp->text);
        cque_free_entry(tmp);
        continue;
      }
      context.debug_command = "< do_top >";
      btech_context_set_command(&queue->server->btech, &context);
      queue_adjust(queue->server->players,
                   game_object_owner(&queue->server->database, object), -1);
      if (!is_halted(&queue->server->database, object)) {
        for (i = 0; i < MAX_GLOBAL_REGS; i++) {
          if (tmp->scr[i]) {
            StringCopy(context.evaluation.registers[i], tmp->scr[i]);
          } else {
            *context.evaluation.registers[i] = '\0';
          }
        }

        command = tmp->comm;

        if (command) {
          while (command) {
            cp = parse_to(queue->server->configuration, &command, ';', 0);
            if (cp && *cp) {
              while (command && (*command == '|')) {
                command++;
                context.evaluation.is_piping = true;
                context.evaluation.pipe_next =
                    alloc_lbuf("process_command.pipe");
                context.evaluation.pipe_cursor = context.evaluation.pipe_next;
                context.evaluation.pipe_object = object;
                process_command(&context, cp, tmp->env, tmp->nargs);
                if (context.evaluation.pipe_output) {
                  free_lbuf(context.evaluation.pipe_output);
                  context.evaluation.pipe_output = nullptr;
                }

                *context.evaluation.pipe_cursor = '\0';
                context.evaluation.pipe_output = context.evaluation.pipe_next;
                cp = parse_to(queue->server->configuration, &command, ';', 0);
              }
              context.evaluation.is_piping = false;
              process_command(&context, cp, tmp->env, tmp->nargs);
              if (context.evaluation.pipe_output) {
                free_lbuf(context.evaluation.pipe_output);
                context.evaluation.pipe_output = nullptr;
                context.evaluation.pipe_next = nullptr;
              }
            }
          }
        }
      }
      btech_context_set_command(&queue->server->btech, previous_context);
      command_context_destroy(&context);
    }
    free(tmp->text);
    cque_free_entry(tmp);
  }

  return count;
}

/*
 * ---------------------------------------------------------------------------
 * * do_ps: tell player what commands they have pending in the queue
 */

static void show_que(EvaluationContext *evaluation, DbRef player, int key,
                     BQUE *queue, RuntimeClock *clock, int *qent,
                     const char *header) {
  BQUE *tmp;
  char *bp, *bufp;
  int i;

  for (tmp = queue; tmp; tmp = tmp->next) {
    (*qent)++;
    if (key == PS_SUMM)
      continue;
    if (*qent == 1)
      notify_printf(evaluation, player, "----- %s Queue -----", header);

    bufp = unparse_object(evaluation->world->database, evaluation, player,
                          tmp->player, 0);
    if (!(key & PS_ALL))
      if ((player !=
           game_object_owner(evaluation->world->database, tmp->player)))
        continue;
    if ((tmp->waittime > 0) &&
        is_good_obj(evaluation->world->database, tmp->sem))
      notify_printf(evaluation, player, "[#%ld/%ld]%s:%s", tmp->sem,
                    tmp->waittime - clock->now, bufp, tmp->comm);
    else if (tmp->waittime > 0)
      notify_printf(evaluation, player, "[%ld]%s:%s",
                    tmp->waittime - clock->now, bufp, tmp->comm);
    else if (is_good_obj(evaluation->world->database, tmp->sem))
      notify_printf(evaluation, player, "[#%ld]%s:%s", tmp->sem, bufp,
                    tmp->comm);
    else
      notify_printf(evaluation, player, "%s:%s", bufp, tmp->comm);

    bp = bufp;
    if (key == PS_LONG) {
      for (i = 0; i < (tmp->nargs); i++) {
        if (tmp->env[i] != nullptr) {
          safe_str("; Arg", bufp, &bp);
          safe_chr((char)(i + '0'), bufp, &bp);
          safe_str("='", bufp, &bp);
          safe_str(tmp->env[i], bufp, &bp);
          safe_chr('\'', bufp, &bp);
        }
      }
      *bp = '\0';
      bp = unparse_object(evaluation->world->database, evaluation, player,
                          tmp->cause, 0);
      notify_printf(evaluation, player, "   Enactor: %s%s", bp, bufp);
      free_lbuf(bp);
    }
    free_lbuf(bufp);
  }
  return;
}

void do_ps(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *target = invocation->first;
  CommandQueue *queue = invocation->context->server->commands;
  DbRef player_targ, obj_targ;
  int pqent, pqtot, wqent, wqtot, sqent, sqtot;
  OBJQE *objq;
  int tempkey;

  /*
   * Figure out what to list the queue for
   */

  if ((key & PS_ALL) && !is_wizard(&queue->server->database, player)) {
    notify(evaluation, player, "Permission denied.");
    return;
  }
  if (!target || !*target) {
    obj_targ = NOTHING;
    if (key & PS_ALL) {
      player_targ = NOTHING;
    } else {
      player_targ = game_object_owner(&queue->server->database, player);
      if (typeof_obj(&queue->server->database, player) != TYPE_PLAYER)
        obj_targ = player;
    }
  } else {
    player_targ = game_object_owner(&queue->server->database, player);
    obj_targ = match_controlled(&invocation->context->match, player, target);
    if (obj_targ == NOTHING)
      return;
    if (key & PS_ALL) {
      notify(evaluation, player, "Can't specify a target and /all");
      return;
    }
    if (typeof_obj(&queue->server->database, obj_targ) == TYPE_PLAYER) {
      player_targ = obj_targ;
      obj_targ = NOTHING;
    }
  }
  tempkey = key;
  key = key & ~PS_ALL;
  switch (key) {
  case PS_BRIEF:
  case PS_SUMM:
  case PS_LONG:
    break;
  default:
    notify(evaluation, player, "Illegal combination of switches.");
    return;
  }

  /*
   * Go do it
   */
  pqent = 0;
  pqtot = 0;
  if (player_targ == NOTHING) {
    objq = queue->head;
    while (objq && (objq = objq->next) != nullptr) {
      pqent = 0;
      show_que(evaluation, player, tempkey, objq->cque, &queue->server->clock,
               &pqent, "PLAYAH");
      pqtot += pqent;
    }
  } else {
    pqent = 0;
    objq = cque_find(queue, player_targ);
    if (objq) {
      show_que(evaluation, player, tempkey, objq->cque, &queue->server->clock,
               &pqent, "PLAYAH");
    }
  }

  wqent = 0;
  sqent = 0;
  wqtot = 0;
  sqtot = 0;
  show_que(evaluation, player, tempkey, queue->wait, &queue->server->clock,
           &wqent, "Wait");
  show_que(evaluation, player, tempkey, queue->semaphore_first,
           &queue->server->clock, &sqent, "Semaphore");

  /*
   * Display stats
   */

  notify_printf(evaluation, player,
                "Totals: Player...%d/%d  Wait...%d/%d  Semaphore...%d/%d",
                pqent, pqtot, wqent, wqtot, sqent, sqtot);
}

/*
 * ---------------------------------------------------------------------------
 * * do_queue: Queue management
 */

void do_queue(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *arg = invocation->first;
  CommandQueue *queue = invocation->context->server->commands;
  BQUE *point;
  int i, ncmds;
  bool was_disabled;

  dprintk("WTF?");
  was_disabled = false;
  if (key == QUEUE_KICK) {
    i = atoi(arg);
    if (!queue->server->configuration->is_dequeue_enabled) {
      was_disabled = true;
      queue->server->configuration->is_dequeue_enabled = true;
      notify(evaluation, player, "Warning: automatic dequeueing is disabled.");
    }
    ncmds = do_top(queue, i);
    if (was_disabled)
      queue->server->configuration->is_dequeue_enabled = false;
    if (!is_quiet(&queue->server->database, player))
      notify_printf(evaluation, player, "%d commands processed.", ncmds);
  } else if (key == QUEUE_WARP) {
    i = atoi(arg);
    if (!queue->server->configuration->is_dequeue_enabled) {
      was_disabled = true;
      queue->server->configuration->is_dequeue_enabled = true;
      notify(evaluation, player, "Warning: automatic dequeueing is disabled.");
    }

    /*
     * Handle the semaphore queue
     */

    for (point = queue->semaphore_first; point; point = point->next) {
      if (point->waittime > 0) {
        point->waittime -= i;
        if (point->waittime <= 0)
          point->waittime = -1;
      }
    }

    do_second(queue);
    if (was_disabled)
      queue->server->configuration->is_dequeue_enabled = false;
    if (is_quiet(&queue->server->database, player))
      return;
    if (i > 0)
      notify_printf(evaluation, player, "WaitQ timer advanced %d seconds.", i);
    else if (i < 0)
      notify_printf(evaluation, player, "WaitQ timer set back %d seconds.", i);
    else
      notify(evaluation, player, "Object queue appended to player queue.");
  }
}
