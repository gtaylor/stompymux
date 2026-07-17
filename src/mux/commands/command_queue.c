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
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"
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

static RedBlackTree obq = nullptr;

static void cque_free_entry(BQUE *entry) {
  if (entry->ev != nullptr)
    event_free(entry->ev);
  free_qentry(entry);
}

static int objqe_compare(DbRef left, DbRef right, void *arg) {
  return (right > left) - (right < left);
}

int cque_init(void) {
  obq = red_black_tree_init(
      (int (*)(void *, void *, void *))(GenericFnPtr)objqe_compare, nullptr);
  return 1;
}

static OBJQE *cque_find(DbRef player) {
  OBJQE *tmp = nullptr;

  if (obq == nullptr) {
    cque_init();
  }

  tmp = red_black_tree_find(obq, (void *)player);

  if (!tmp && is_good_obj(player)) {
    tmp = malloc(sizeof(OBJQE));
    tmp->obj = player;
    tmp->cque = nullptr;
    tmp->ctail = nullptr;
    tmp->next = nullptr;
    tmp->queued = 0;
    tmp->wait_que = nullptr;
    tmp->pending_que = nullptr;
    red_black_tree_insert(obq, (void *)player, tmp);
  }

  return tmp;
}

static BQUE *cque_deque(DbRef player) {
  OBJQE *tmp;
  BQUE *cmd;

  tmp = cque_find(player);
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

static void cque_enqueue(DbRef player, BQUE *cmd) {
  BQUE *point, *trail;
  struct timeval tv;
  OBJQE *tmp;

  cmd->next = nullptr;

  tv.tv_sec = cmd->waittime - mudstate.now;
  tv.tv_usec = 0;

  if (cmd->sem == NOTHING) {
    /*
     * No semaphore, put on wait queue if wait value specified.
     * Otherwise put on the normal queue.
     */

    if (cmd->waittime <= mudstate.now) {
      cmd->waittime = 0;
      tmp = cque_find(player);

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
        if (!mudstate.qhead) {
          mudstate.qhead = mudstate.qtail = tmp;
          tmp->next = nullptr;
        } else {
          mudstate.qtail->next = tmp;
          mudstate.qtail = tmp;
          mudstate.qtail->next = nullptr;
        }
        tmp->queued = 1;
      }
    } else {
      evtimer_add(cmd->ev, &tv);
      for (point = mudstate.qwait, trail = nullptr;
           point && point->waittime <= cmd->waittime; point = point->next) {
        trail = point;
      }
      cmd->next = point;
      if (trail != nullptr)
        trail->next = cmd;
      else
        mudstate.qwait = cmd;
    }
  } else {
    cmd->next = nullptr;
    if (mudstate.qsemlast != nullptr)
      mudstate.qsemlast->next = cmd;
    else
      mudstate.qsemfirst = cmd;
    mudstate.qsemlast = cmd;
  }
}

static void wakeup_wait_que(evutil_socket_t fd, short event, void *arg) {
  BQUE *pending = (BQUE *)arg;
  BQUE *point;

  if (mudstate.qwait == pending) {
    mudstate.qwait = pending->next;
  } else {
    for (point = mudstate.qwait; point; point = point->next) {
      if (point->next == pending) {
        point->next = point->next->next;
        break;
      }
    }
  }

  pending->waittime = 0;
  cque_enqueue(pending->player, pending);
}

/*
 * ---------------------------------------------------------------------------
 * * add_to: Adjust an object's queue or semaphore count.
 */

static int add_to(DbRef player, int am, int attrnum) {
  int num;
  long aflags;
  DbRef aowner;
  char buff[20] = {0};
  char *atr_gotten;

  num = atoi(atr_gotten = attribute_get(player, attrnum, &aowner, &aflags));
  free_lbuf(atr_gotten);
  num += am;
  if (num)
    snprintf(buff, sizeof(buff), "%d", num);
  else
    *buff = '\0';
  attribute_add_raw(player, attrnum, buff);
  return (num);
}

/*
 * ---------------------------------------------------------------------------
 * * que_want: Do we want this queue entry?
 */

static int que_want(BQUE *entry, DbRef ptarg, DbRef otarg) {
  if ((ptarg != NOTHING) && (ptarg != obj_owner(entry->player)))
    return 0;
  if ((otarg != NOTHING) && (otarg != entry->player))
    return 0;
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * halt_que: Remove all queued commands from a certain player
 */

int halt_que(DbRef player, DbRef object) {
  BQUE *trail, *point, *next;
  OBJQE *pque;

  int numhalted;

  numhalted = 0;

  /* Player's que */
  // XXX: nuke queu

  pque = cque_find(player);
  if (pque && pque->cque) {
    while ((point = cque_deque(player)) != nullptr) {
      free(point->text);
      point->text = nullptr;
      cque_free_entry(point);
      point = nullptr;
      numhalted++;
    }
  }
  pque = cque_find(object);
  if (pque && pque->cque) {
    while ((point = cque_deque(object)) != nullptr) {
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

  for (point = mudstate.qwait, trail = nullptr; point; point = next)
    if (que_want(point, player, object)) {
      numhalted++;
      if (trail)
        trail->next = next = point->next;
      else
        mudstate.qwait = next = point->next;
      if (event_pending(point->ev, EV_TIMEOUT, nullptr))
        event_del(point->ev);
      free(point->text);
      cque_free_entry(point);
    } else
      next = (trail = point)->next;

  /*
   * Semaphore queue
   */

  for (point = mudstate.qsemfirst, trail = nullptr; point; point = next)
    if (que_want(point, player, object)) {
      numhalted++;
      if (trail)
        trail->next = next = point->next;
      else
        mudstate.qsemfirst = next = point->next;
      if (point == mudstate.qsemlast)
        mudstate.qsemlast = trail;
      add_to(point->sem, -1, point->attr);
      free(point->text);
      cque_free_entry(point);
    } else
      next = (trail = point)->next;

  if (player == NOTHING)
    player = obj_owner(object);
  if (object == NOTHING)
    queue_set(player, 0);
  else
    queue_adjust(player, -numhalted);
  return numhalted;
}

/*
 * ---------------------------------------------------------------------------
 * * do_halt: Command interface to halt_que.
 */

void do_halt(DbRef player, DbRef cause, int key, char *target) {
  DbRef player_targ, obj_targ;
  int numhalted;

  if ((key & HALT_ALL) && !is_wizard(player)) {
    notify(player, "Permission denied.");
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
      player_targ = obj_owner(player);
      if (typeof_obj(player) != TYPE_PLAYER)
        obj_targ = player;
    }
  } else {
    if (is_wizard(player))
      obj_targ = match_thing(player, target);
    else
      obj_targ = match_controlled(player, target);

    if (obj_targ == NOTHING)
      return;
    if (key & HALT_ALL) {
      notify(player, "Can't specify a target and /all");
      return;
    }
    if (typeof_obj(obj_targ) == TYPE_PLAYER) {
      player_targ = obj_targ;
      obj_targ = NOTHING;
    } else {
      player_targ = NOTHING;
    }
  }

  numhalted = halt_que(player_targ, obj_targ);
  if (is_quiet(player))
    return;
  if (numhalted == 1)
    notify(obj_owner(player), "1 queue entries removed.");
  else
    notify_printf(obj_owner(player), "%d queue entries removed.", numhalted);
}

/*
 * ---------------------------------------------------------------------------
 * * nfy_que: Notify commands from the queue and perform or discard them.
 */

int nfy_que(DbRef sem, int attr, int key, int count) {
  BQUE *point, *trail, *next;
  int num;
  long aflags;
  DbRef aowner;
  char *str;

  if (attr) {
    str = attribute_get(sem, attr, &aowner, &aflags);
    num = atoi(str);
    free_lbuf(str);
  } else {
    num = 1;
  }

  if (num > 0) {
    num = 0;
    for (point = mudstate.qsemfirst, trail = nullptr; point; point = next) {
      if ((point->sem == sem) && ((point->attr == attr) || !attr)) {
        num++;
        if (trail)
          trail->next = next = point->next;
        else
          mudstate.qsemfirst = next = point->next;
        if (point == mudstate.qsemlast)
          mudstate.qsemlast = trail;

        /*
         * Either run or discard the command
         */

        if (key != NFY_DRAIN) {
          point->sem = NOTHING;
          point->waittime = 0;
          cque_enqueue(point->player, point);
        } else {
          queue_adjust(obj_owner(point->player), -1);
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
    add_to(sem, -count, attr);
  else
    attribute_clear(sem, attr);

  return num;
}

/*
 * ---------------------------------------------------------------------------
 * * do_notify: Command interface to nfy_que
 */

void do_notify(DbRef player, DbRef cause, int key, char *what, char *count) {
  DbRef thing, aowner;
  int loccount, attr = -1;
  long aflags;
  Attribute *ap;
  char *obj;

  obj = parse_to(&what, '/', 0);
  init_match(player, obj, NOTYPE);
  match_everything(0);

  if ((thing = noisy_match_result()) < 0) {
    notify(player, "No match.");
  } else if (!is_controls(player, thing)) {
    notify(player, "Permission denied.");
  } else {
    if (!what || !*what) {
      ap = nullptr;
    } else {
      ap = attribute_by_name(what);
    }

    if (!ap) {
      attr = A_SEMAPHORE;
    } else {
      /* Do they have permission to set this attribute? */
      attribute_parent_get_info(thing, ap->number, &aowner, &aflags);
      if (set_attr(player, thing, ap, aflags)) {
        attr = ap->number;
      } else {
        notify_quiet(player, "Permission denied.");
        return;
      }
    }

    if (count && *count)
      loccount = atoi(count);
    else
      loccount = 1;
    if (loccount > 0) {
      nfy_que(thing, attr, key, loccount);
      if (!(is_quiet(player) || is_quiet(thing))) {
        if (key == NFY_DRAIN)
          notify_quiet(player, "Drained.");
        else
          notify_quiet(player, "Notified.");
      }
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * setup_que: Set up a queue entry.
 */

static BQUE *setup_que(DbRef player, DbRef cause, char *command, char *args[],
                       int nargs, char *sargs[]) {
  int a;
  size_t tlen;
  BQUE *tmp;
  char *tptr;

  /*
   * Can we run commands at all?
   */

  if (is_halted(player))
    return nullptr;

  /*
   * Wizards and their objs may queue up to db_top+1 cmds. Players are
   * * * * * * * limited to QUEUE_QUOTA. -mnp
   */

  a = queue_maximum(obj_owner(player));
  if (queue_adjust(obj_owner(player), 1) > a) {
    notify(obj_owner(player),
           "Run away objects: too many commands queued.  Halted.");
    halt_que(obj_owner(player), NOTHING);

    /*
     * halt also means no command execution allowed
     */
    s_halted(player);
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

  tmp->ev = evtimer_new(server_lifecycle_event_base(), wakeup_wait_que, tmp);
  if (tmp->ev == nullptr) {
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
  return tmp;
}

/*
 * ---------------------------------------------------------------------------
 * * wait_que: Add commands to the wait or semaphore queues.
 */

void wait_que(DbRef player, DbRef cause, int wait, DbRef sem, int attr,
              char *command, char *args[], int nargs, char *sargs[]) {
  BQUE *cmd;
  if (mudconf.control_flags & CF_INTERP)
    cmd = setup_que(player, cause, command, args, nargs, sargs);
  else
    cmd = nullptr;

  if (cmd == nullptr) {
    return;
  }

  if (wait > 0) {
    cmd->waittime = (int)(mudstate.now + wait);
  } else {
    cmd->waittime = 0;
  }

  cmd->sem = sem;
  cmd->attr = attr;

  cque_enqueue(player, cmd);
}

/*
 * ---------------------------------------------------------------------------
 * * do_wait: Command interface to wait_que
 */

void do_wait(DbRef player, DbRef cause, int key, char *event, char *cmd,
             char *cargs[], int ncargs) {
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
    wait_que(player, cause, howlong, NOTHING, 0, cmd, cargs, ncargs,
             mudstate.global_regs);
    return;
  }
  /*
   * Semaphore wait with optional timeout
   */

  what = parse_to(&event, '/', 0);
  init_match(player, what, NOTYPE);
  match_everything(0);

  thing = noisy_match_result();
  if (!is_good_obj(thing)) {
    notify(player, "No match.");
  } else if (!is_controls(player, thing)) {
    notify(player, "Permission denied.");
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
      ap = attribute_by_name(event);
      if (!ap) {
        attr = mkattr(event);
        if (attr <= 0) {
          notify_quiet(player, "Invalid attribute.");
          return;
        }
        ap = attribute_by_number(attr);
      }
      attribute_parent_get_info(thing, ap->number, &aowner, &aflags);
      if (attr && set_attr(player, thing, ap, aflags)) {
        attr = ap->number;
        howlong = 0;
      } else {
        notify_quiet(player, "Permission denied.");
        return;
      }
    }

    num = add_to(thing, 1, attr);
    if (num <= 0) {

      /*
       * thing over-notified, run the command immediately
       */

      thing = NOTHING;
      howlong = 0;
    }
    wait_que(player, cause, howlong, thing, attr, cmd, cargs, ncargs,
             mudstate.global_regs);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_second: Check the wait and semaphore queues for commands to remove.
 */

void do_second(void) {
  BQUE *trail, *point, *next;
  const char *cmdsave;

  /*
   * move contents of low priority queue onto end of normal one this
   * helps to keep objects from getting out of control since
   * its affects on other objects happen only after one
   * second  this should allow @halt to be type before
   * getting blown away  by scrolling text
   */

  if ((mudconf.control_flags & CF_DEQUEUE) == 0)
    return;

  cmdsave = mudstate.debug_cmd;
  mudstate.debug_cmd = "< do_second >";

  /*
   * Note: the point->waittime test would be 0 except the command is
   * being put in the low priority queue to be done in one
   * second anyways
   */

  /*
   * Check the semaphore queue for expired timed-waits
   */

  for (point = mudstate.qsemfirst, trail = nullptr; point; point = next) {
    if (point->waittime == 0) {
      next = (trail = point)->next;
      continue; /*
                 * Skip if not timed-wait
                 */
    }
    if (point->waittime <= mudstate.now) {
      if (trail != nullptr)
        trail->next = next = point->next;
      else
        mudstate.qsemfirst = next = point->next;
      if (point == mudstate.qsemlast)
        mudstate.qsemlast = trail;
      add_to(point->sem, -1, point->attr);
      point->sem = NOTHING;
      point->waittime = 0;
      printk("promoting, %ld/%s", point->player, point->comm);
      cque_enqueue(point->player, point);
    } else
      next = (trail = point)->next;
  }
  mudstate.debug_cmd = cmdsave;
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * do_top: Execute the command at the top of the queue
 */

int do_top(int ncmds) {
  BQUE *tmp;
  DbRef object;
  int count, i;
  char *command, *cp;
  const char *cmdsave;

  if ((mudconf.control_flags & CF_DEQUEUE) == 0)
    return 0;

  cmdsave = mudstate.debug_cmd;
  mudstate.debug_cmd = "< do_top >";

  if (!mudstate.qhead)
    return 0;

  count = 0;

  while (count < ncmds && mudstate.qhead) {
    if (!mudstate.qhead)
      break;

    object = mudstate.qhead->obj;
    tmp = cque_deque(object);

    if (!mudstate.qhead->cque) {
      mudstate.qhead->queued = 0;
      mudstate.qhead = mudstate.qhead->next;
      if (mudstate.qhead == nullptr)
        mudstate.qtail = nullptr;
    } else {
      mudstate.qtail->next = mudstate.qhead;
      mudstate.qtail = mudstate.qtail->next;
      mudstate.qhead = mudstate.qhead->next;
      mudstate.qtail->next = nullptr;
    }
    if (!tmp)
      continue;

    dassert(tmp);
    count++;
    if ((object >= 0) && !is_going(object)) {
      mudstate.curr_enactor = tmp->cause;
      mudstate.curr_player = object;
      mudstate.curr_descriptor = nullptr;
      queue_adjust(obj_owner(object), -1);
      if (!is_halted(object)) {
        for (i = 0; i < MAX_GLOBAL_REGS; i++) {
          if (tmp->scr[i]) {
            StringCopy(mudstate.global_regs[i], tmp->scr[i]);
          } else {
            *mudstate.global_regs[i] = '\0';
          }
        }

        command = tmp->comm;

        if (command) {
          while (command) {
            cp = parse_to(&command, ';', 0);
            if (cp && *cp) {
              while (command && (*command == '|')) {
                command++;
                mudstate.inpipe = 1;
                mudstate.poutnew = alloc_lbuf("process_command.pipe");
                mudstate.poutbufc = mudstate.poutnew;
                mudstate.poutobj = object;
                process_command(object, tmp->cause, 0, cp, tmp->env,
                                tmp->nargs);
                if (mudstate.pout) {
                  free_lbuf(mudstate.pout);
                  mudstate.pout = nullptr;
                }

                *mudstate.poutbufc = '\0';
                mudstate.pout = mudstate.poutnew;
                cp = parse_to(&command, ';', 0);
              }
              mudstate.inpipe = 0;
              process_command(object, tmp->cause, 0, cp, tmp->env, tmp->nargs);
              if (mudstate.pout) {
                free_lbuf(mudstate.pout);
                mudstate.pout = nullptr;
              }
            }
          }
        }
      }
    }
    free(tmp->text);
    cque_free_entry(tmp);
  }

  for (i = 0; i < MAX_GLOBAL_REGS; i++)
    *mudstate.global_regs[i] = '\0';
  mudstate.debug_cmd = cmdsave;
  return count;
}

/*
 * ---------------------------------------------------------------------------
 * * do_ps: tell player what commands they have pending in the queue
 */

static void show_que(DbRef player, int key, BQUE *queue, int *qent,
                     const char *header) {
  BQUE *tmp;
  char *bp, *bufp;
  int i;

  for (tmp = queue; tmp; tmp = tmp->next) {
    (*qent)++;
    if (key == PS_SUMM)
      continue;
    if (*qent == 1)
      notify_printf(player, "----- %s Queue -----", header);

    bufp = unparse_object(player, tmp->player, 0);
    if (!(key & PS_ALL))
      if ((player != obj_owner(tmp->player)))
        continue;
    if ((tmp->waittime > 0) && (is_good_obj(tmp->sem)))
      notify_printf(player, "[#%ld/%ld]%s:%s", tmp->sem,
                    tmp->waittime - mudstate.now, bufp, tmp->comm);
    else if (tmp->waittime > 0)
      notify_printf(player, "[%ld]%s:%s", tmp->waittime - mudstate.now, bufp,
                    tmp->comm);
    else if (is_good_obj(tmp->sem))
      notify_printf(player, "[#%ld]%s:%s", tmp->sem, bufp, tmp->comm);
    else
      notify_printf(player, "%s:%s", bufp, tmp->comm);

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
      bp = unparse_object(player, tmp->cause, 0);
      notify_printf(player, "   Enactor: %s%s", bp, bufp);
      free_lbuf(bp);
    }
    free_lbuf(bufp);
  }
  return;
}

void do_ps(DbRef player, DbRef cause, int key, char *target) {
  DbRef player_targ, obj_targ;
  int pqent, pqtot, wqent, wqtot, sqent, sqtot;
  OBJQE *objq;
  int tempkey;

  /*
   * Figure out what to list the queue for
   */

  if ((key & PS_ALL) && !is_wizard(player)) {
    notify(player, "Permission denied.");
    return;
  }
  if (!target || !*target) {
    obj_targ = NOTHING;
    if (key & PS_ALL) {
      player_targ = NOTHING;
    } else {
      player_targ = obj_owner(player);
      if (typeof_obj(player) != TYPE_PLAYER)
        obj_targ = player;
    }
  } else {
    player_targ = obj_owner(player);
    obj_targ = match_controlled(player, target);
    if (obj_targ == NOTHING)
      return;
    if (key & PS_ALL) {
      notify(player, "Can't specify a target and /all");
      return;
    }
    if (typeof_obj(obj_targ) == TYPE_PLAYER) {
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
    notify(player, "Illegal combination of switches.");
    return;
  }

  /*
   * Go do it
   */
  pqent = 0;
  pqtot = 0;
  if (player_targ == NOTHING) {
    objq = mudstate.qhead;
    while (objq && (objq = objq->next) != nullptr) {
      pqent = 0;
      show_que(player, tempkey, objq->cque, &pqent, "PLAYAH");
      pqtot += pqent;
    }
  } else {
    pqent = 0;
    objq = cque_find(player_targ);
    if (objq) {
      show_que(player, tempkey, objq->cque, &pqent, "PLAYAH");
    }
  }

  wqent = 0;
  sqent = 0;
  wqtot = 0;
  sqtot = 0;
  show_que(player, tempkey, mudstate.qwait, &wqent, "Wait");
  show_que(player, tempkey, mudstate.qsemfirst, &sqent, "Semaphore");

  /*
   * Display stats
   */

  notify_printf(player,
                "Totals: Player...%d/%d  Wait...%d/%d  Semaphore...%d/%d",
                pqent, pqtot, wqent, wqtot, sqent, sqtot);
}

/*
 * ---------------------------------------------------------------------------
 * * do_queue: Queue management
 */

void do_queue(DbRef player, DbRef cause, int key, char *arg) {
  BQUE *point;
  int i, ncmds, was_disabled;

  dprintk("WTF?");
  was_disabled = 0;
  if (key == QUEUE_KICK) {
    i = atoi(arg);
    if ((mudconf.control_flags & CF_DEQUEUE) == 0) {
      was_disabled = 1;
      mudconf.control_flags |= CF_DEQUEUE;
      notify(player, "Warning: automatic dequeueing is disabled.");
    }
    ncmds = do_top(i);
    if (was_disabled)
      mudconf.control_flags &= ~CF_DEQUEUE;
    if (!is_quiet(player))
      notify_printf(player, "%d commands processed.", ncmds);
  } else if (key == QUEUE_WARP) {
    i = atoi(arg);
    if ((mudconf.control_flags & CF_DEQUEUE) == 0) {
      was_disabled = 1;
      mudconf.control_flags |= CF_DEQUEUE;
      notify(player, "Warning: automatic dequeueing is disabled.");
    }

    /*
     * Handle the semaphore queue
     */

    for (point = mudstate.qsemfirst; point; point = point->next) {
      if (point->waittime > 0) {
        point->waittime -= i;
        if (point->waittime <= 0)
          point->waittime = -1;
      }
    }

    do_second();
    if (was_disabled)
      mudconf.control_flags &= ~CF_DEQUEUE;
    if (is_quiet(player))
      return;
    if (i > 0)
      notify_printf(player, "WaitQ timer advanced %d seconds.", i);
    else if (i < 0)
      notify_printf(player, "WaitQ timer set back %d seconds.", i);
    else
      notify(player, "Object queue appended to player queue.");
  }
}
