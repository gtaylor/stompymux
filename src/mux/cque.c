/*
 * cque.c -- commands and functions for manipulating the command queue
 */

#include "config.h"

#include <limits.h>
#include <signal.h>
#include <sqlite3.h>

#include "alloc.h"
#include "attrs.h"
#include "command.h"
#include "config.h"
#include "cque.h"
#include "db.h"
#include "externs.h"
#include "flags.h"
#include "functions.h"
#include "interface.h"
#include "match.h"
#include "mudconf.h"
#include "pcache.h"
#include "powers.h"
#include "rbtab.h"

static rbtree obq = NULL;

static int objqe_compare(dbref left, dbref right, void *arg) {
  return (right - left);
}

int cque_init(void) {
  obq = rb_init((void *)objqe_compare, NULL);
  return 1;
};

static OBJQE *cque_find(dbref player) {
  OBJQE *tmp = NULL;

  if (obq == NULL) {
    cque_init();
  }

  tmp = rb_find(obq, (void *)player);

  if (!tmp && Good_obj(player)) {
    tmp = malloc(sizeof(OBJQE));
    tmp->obj = player;
    tmp->cque = NULL;
    tmp->ctail = NULL;
    tmp->next = NULL;
    tmp->queued = 0;
    tmp->wait_que = NULL;
    tmp->pending_que = NULL;
    rb_insert(obq, (void *)player, tmp);
  }

  return tmp;
}

static BQUE *cque_deque(dbref player) {
  OBJQE *tmp;
  BQUE *cmd;

  tmp = cque_find(player);
  if (!tmp)
    return NULL;

  dassert(tmp);

  if (!tmp->cque)
    return NULL;

  cmd = tmp->cque;
  if (!cmd->next) {
    tmp->cque = tmp->ctail = NULL;
  } else {
    tmp->cque = cmd->next;
  }
  return cmd;
}

static void cque_enqueue(dbref player, BQUE *cmd) {
  BQUE *point, *trail;
  struct timeval tv;
  OBJQE *tmp;

  cmd->next = NULL;

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
        cmd->next = NULL;
      } else {
        tmp->ctail->next = cmd;
        tmp->ctail = cmd;
        tmp->ctail->next = NULL;
      }

      if (!tmp->queued) {
        if (!mudstate.qhead) {
          mudstate.qhead = mudstate.qtail = tmp;
          tmp->next = NULL;
        } else {
          mudstate.qtail->next = tmp;
          mudstate.qtail = tmp;
          mudstate.qtail->next = NULL;
        }
        tmp->queued = 1;
      }
    } else {
      evtimer_add(&cmd->ev, &tv);
      for (point = mudstate.qwait, trail = NULL;
           point && point->waittime <= cmd->waittime; point = point->next) {
        trail = point;
      }
      cmd->next = point;
      if (trail != NULL)
        trail->next = cmd;
      else
        mudstate.qwait = cmd;
    }
  } else {
    cmd->next = NULL;
    if (mudstate.qsemlast != NULL)
      mudstate.qsemlast->next = cmd;
    else
      mudstate.qsemfirst = cmd;
    mudstate.qsemlast = cmd;
  }
}

static void wakeup_wait_que(int fd, short event, void *arg) {
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

typedef struct cque_restart_store_context CQUE_RESTART_STORE_CONTEXT;
struct cque_restart_store_context {
  sqlite3 *sqlite;
  sqlite3_stmt *object;
  sqlite3_stmt *entry;
  sqlite3_stmt *environment;
  sqlite3_stmt *registers;
  int result;
};

static int cque_sqlite_step(sqlite3_stmt *statement) {
  if (sqlite3_step(statement) != SQLITE_DONE ||
      sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

static int cque_sqlite_bind_int(sqlite3_stmt *statement, int index,
                                long value) {
  return sqlite3_bind_int64(statement, index, (sqlite3_int64)value) == SQLITE_OK
             ? 0
             : -1;
}

static int cque_restart_store_entry(CQUE_RESTART_STORE_CONTEXT *context,
                                    int queue_type, dbref owner, int position,
                                    BQUE *bqe) {
  sqlite3_int64 entry_id;
  long delay;
  int index;

  delay = bqe->waittime - mudstate.now;
  if (delay < 0)
    delay = 0;
  if (cque_sqlite_bind_int(context->entry, 1, queue_type) < 0 ||
      cque_sqlite_bind_int(context->entry, 2, owner) < 0 ||
      cque_sqlite_bind_int(context->entry, 3, position) < 0 ||
      cque_sqlite_bind_int(context->entry, 4, bqe->player) < 0 ||
      cque_sqlite_bind_int(context->entry, 5, bqe->cause) < 0 ||
      cque_sqlite_bind_int(context->entry, 6, bqe->sem) < 0 ||
      cque_sqlite_bind_int(context->entry, 7, delay) < 0 ||
      cque_sqlite_bind_int(context->entry, 8, bqe->attr) < 0 ||
      sqlite3_bind_text(context->entry, 9, bqe->text, -1, SQLITE_TRANSIENT) !=
          SQLITE_OK ||
      sqlite3_bind_text(context->entry, 10, bqe->comm, -1, SQLITE_TRANSIENT) !=
          SQLITE_OK ||
      cque_sqlite_bind_int(context->entry, 11, bqe->nargs) < 0 ||
      cque_sqlite_step(context->entry) < 0)
    return -1;
  entry_id = sqlite3_last_insert_rowid(context->sqlite);
  if (entry_id <= 0)
    return -1;
  for (index = 0; index < NUM_ENV_VARS; index++) {
    if (cque_sqlite_bind_int(context->environment, 1, entry_id) < 0 ||
        cque_sqlite_bind_int(context->environment, 2, index) < 0 ||
        sqlite3_bind_text(context->environment, 3, bqe->env[index], -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        cque_sqlite_step(context->environment) < 0 ||
        cque_sqlite_bind_int(context->registers, 1, entry_id) < 0 ||
        cque_sqlite_bind_int(context->registers, 2, index) < 0 ||
        sqlite3_bind_text(context->registers, 3, bqe->scr[index], -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        cque_sqlite_step(context->registers) < 0)
      return -1;
  }
  return 0;
}

static int cque_restart_store_object(void *key, void *data, int depth,
                                     void *argument) {
  CQUE_RESTART_STORE_CONTEXT *context = argument;
  OBJQE *object = data;
  BQUE *entry;
  int position;

  (void)key;
  (void)depth;
  if (context->result < 0)
    return 0;
  if (cque_sqlite_bind_int(context->object, 1, object->obj) < 0 ||
      cque_sqlite_step(context->object) < 0) {
    context->result = -1;
    return 0;
  }
  position = 0;
  for (entry = object->cque; entry; entry = entry->next) {
    if (cque_restart_store_entry(context, 0, object->obj, position++, entry) <
        0) {
      context->result = -1;
      return 0;
    }
  }
  return 1;
}

int cque_restart_store(sqlite3 *sqlite) {
  CQUE_RESTART_STORE_CONTEXT context;
  BQUE *entry;
  int position;
  int result;

  memset(&context, 0, sizeof(context));
  context.sqlite = sqlite;
  context.result = -1;
  if (sqlite3_prepare_v2(
          sqlite, "INSERT INTO restart_queue_objects (owner_dbref) VALUES (?);",
          -1, &context.object, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO restart_queue_entries "
          "(queue_type, owner_dbref, position, player_dbref, cause_dbref, "
          "semaphore_dbref, wait_delay, attribute_number, text, command, "
          "nargs) "
          "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &context.entry, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO restart_queue_env "
                         "(entry_id, position, value) VALUES (?, ?, ?);",
                         -1, &context.environment, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO restart_queue_scr "
                         "(entry_id, position, value) VALUES (?, ?, ?);",
                         -1, &context.registers, NULL) != SQLITE_OK) {
    result = -1;
  } else {
    if (!obq)
      cque_init();
    context.result = 0;
    if (rb_size(obq) > 0)
      rb_walk(obq, WALK_INORDER, cque_restart_store_object, &context);
    position = 0;
    for (entry = mudstate.qwait; context.result == 0 && entry;
         entry = entry->next)
      context.result =
          cque_restart_store_entry(&context, 1, NOTHING, position++, entry);
    position = 0;
    for (entry = mudstate.qsemfirst; context.result == 0 && entry;
         entry = entry->next)
      context.result =
          cque_restart_store_entry(&context, 2, NOTHING, position++, entry);
    result = context.result;
  }
  sqlite3_finalize(context.object);
  sqlite3_finalize(context.entry);
  sqlite3_finalize(context.environment);
  sqlite3_finalize(context.registers);
  return result;
}

static int cque_sqlite_column_long(sqlite3_stmt *statement, int column,
                                   long *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < LONG_MIN || number > LONG_MAX)
    return -1;
  *value = (long)number;
  return 0;
}

static int cque_restart_load_values(sqlite3 *sqlite, int environment,
                                    sqlite3_int64 entry_id, char **values) {
  sqlite3_stmt *statement;
  const unsigned char *text;
  int index;
  long position;
  int result;

  statement = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               environment ? "SELECT position, value FROM restart_queue_env "
                             "WHERE entry_id = ? ORDER BY position;"
                           : "SELECT position, value FROM restart_queue_scr "
                             "WHERE entry_id = ? ORDER BY position;",
               -1, &statement, NULL) == SQLITE_OK &&
                   sqlite3_bind_int64(statement, 1, entry_id) == SQLITE_OK
               ? 0
               : -1;
  index = 0;
  while (result == 0 && sqlite3_step(statement) == SQLITE_ROW) {
    if (cque_sqlite_column_long(statement, 0, &position) < 0 ||
        position != index || index >= NUM_ENV_VARS ||
        (sqlite3_column_type(statement, 1) != SQLITE_TEXT &&
         sqlite3_column_type(statement, 1) != SQLITE_NULL)) {
      result = -1;
    } else {
      text = sqlite3_column_type(statement, 1) == SQLITE_NULL
                 ? NULL
                 : sqlite3_column_text(statement, 1);
      if ((text && sqlite3_column_bytes(statement, 1) < 0) ||
          (text && (int)strlen((const char *)text) !=
                       sqlite3_column_bytes(statement, 1)))
        result = -1;
      else if (text && !(values[index] = strdup((const char *)text)))
        result = -1;
    }
    index++;
  }
  if (result == 0 && sqlite3_errcode(sqlite) != SQLITE_OK &&
      sqlite3_errcode(sqlite) != SQLITE_DONE)
    result = -1;
  if (result == 0 && index != NUM_ENV_VARS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

static int cque_restart_load_entry(sqlite3 *sqlite, sqlite3_stmt *statement,
                                   dbref owner) {
  BQUE *entry;
  long value;
  sqlite3_int64 entry_id;
  const unsigned char *text;
  int column;
  int result;

  entry = calloc(1, sizeof(BQUE));
  if (!entry)
    return -1;
  result = -1;
  if (sqlite3_column_type(statement, 0) == SQLITE_INTEGER &&
      sqlite3_column_type(statement, 3) == SQLITE_INTEGER &&
      sqlite3_column_type(statement, 4) == SQLITE_INTEGER &&
      sqlite3_column_type(statement, 5) == SQLITE_INTEGER &&
      sqlite3_column_type(statement, 6) == SQLITE_INTEGER &&
      sqlite3_column_type(statement, 7) == SQLITE_INTEGER &&
      sqlite3_column_type(statement, 10) == SQLITE_INTEGER) {
    entry_id = sqlite3_column_int64(statement, 0);
    if (entry_id <= 0 || cque_sqlite_column_long(statement, 3, &value) < 0)
      goto done;
    entry->player = value;
    if (!Good_obj(entry->player))
      goto done;
    if (cque_sqlite_column_long(statement, 4, &value) < 0)
      goto done;
    entry->cause = value;
    if (cque_sqlite_column_long(statement, 5, &value) < 0)
      goto done;
    entry->sem = value;
    if (cque_sqlite_column_long(statement, 6, &value) < 0 || value < 0 ||
        value > INT_MAX - mudstate.now)
      goto done;
    entry->waittime = mudstate.now + value;
    if (cque_sqlite_column_long(statement, 7, &value) < 0)
      goto done;
    entry->attr = value;
    for (column = 8; column <= 9; column++) {
      if (sqlite3_column_type(statement, column) == SQLITE_NULL)
        text = NULL;
      else if (sqlite3_column_type(statement, column) == SQLITE_TEXT)
        text = sqlite3_column_text(statement, column);
      else
        goto done;
      if (text && (sqlite3_column_bytes(statement, column) < 0 ||
                   (int)strlen((const char *)text) !=
                       sqlite3_column_bytes(statement, column)))
        goto done;
      if (column == 8)
        entry->text = text ? strdup((const char *)text) : NULL;
      else
        entry->comm = text ? strdup((const char *)text) : NULL;
      if (text && (column == 8 ? !entry->text : !entry->comm))
        goto done;
    }
    if (cque_sqlite_column_long(statement, 10, &value) < 0 || value < 0 ||
        value > INT_MAX)
      goto done;
    entry->nargs = value;
    if (cque_restart_load_values(sqlite, 1, entry_id, entry->env) < 0 ||
        cque_restart_load_values(sqlite, 0, entry_id, entry->scr) < 0)
      goto done;
    evtimer_set(&entry->ev, wakeup_wait_que, entry);
    cque_enqueue(owner, entry);
    return 0;
  }
done:
  for (column = 0; column < NUM_ENV_VARS; column++) {
    free(entry->env[column]);
    free(entry->scr[column]);
  }
  free(entry->text);
  free(entry->comm);
  free(entry);
  return result;
}

int cque_restart_load(sqlite3 *sqlite) {
  sqlite3_stmt *objects;
  sqlite3_stmt *entries;
  long owner;
  long queue_type;
  long position;
  long expected_position;
  int result;
  int step;

  objects = NULL;
  entries = NULL;
  result =
      sqlite3_prepare_v2(sqlite,
                         "SELECT owner_dbref FROM restart_queue_objects "
                         "ORDER BY owner_dbref;",
                         -1, &objects, NULL) == SQLITE_OK &&
              sqlite3_prepare_v2(
                  sqlite,
                  "SELECT entry_id, queue_type, owner_dbref, player_dbref, "
                  "cause_dbref, semaphore_dbref, wait_delay, "
                  "attribute_number, text, command, nargs, position "
                  "FROM restart_queue_entries "
                  "ORDER BY queue_type, owner_dbref, position;",
                  -1, &entries, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(objects)) == SQLITE_ROW) {
    if (cque_sqlite_column_long(objects, 0, &owner) < 0 || !Good_obj(owner) ||
        !cque_find(owner))
      result = -1;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  queue_type = -1;
  owner = LONG_MIN;
  expected_position = 0;
  while (result == 0 && (step = sqlite3_step(entries)) == SQLITE_ROW) {
    long entry_queue_type;
    long entry_owner;

    if (cque_sqlite_column_long(entries, 1, &entry_queue_type) < 0 ||
        cque_sqlite_column_long(entries, 2, &entry_owner) < 0 ||
        cque_sqlite_column_long(entries, 11, &position) < 0 ||
        entry_queue_type < 0 || entry_queue_type > 2 ||
        (entry_queue_type == 0 && !Good_obj(entry_owner)) ||
        (entry_queue_type != 0 && entry_owner != NOTHING)) {
      result = -1;
    } else {
      if (entry_queue_type != queue_type || entry_owner != owner) {
        queue_type = entry_queue_type;
        owner = entry_owner;
        expected_position = 0;
      }
      if (position != expected_position++)
        result = -1;
      else if (cque_restart_load_entry(
                   sqlite, entries,
                   queue_type == 0 ? (dbref)owner
                                   : sqlite3_column_int64(entries, 3)) < 0)
        result = -1;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(objects);
  sqlite3_finalize(entries);
  return result;
}

/*
 * ---------------------------------------------------------------------------
 * * add_to: Adjust an object's queue or semaphore count.
 */

static int add_to(dbref player, int am, int attrnum) {
  int num;
  long aflags;
  dbref aowner;
  char buff[20] = {0};
  char *atr_gotten;

  num = atoi(atr_gotten = atr_get(player, attrnum, &aowner, &aflags));
  free_lbuf(atr_gotten);
  num += am;
  if (num)
    snprintf(buff, sizeof(buff), "%d", num);
  else
    *buff = '\0';
  atr_add_raw(player, attrnum, buff);
  return (num);
}

/*
 * ---------------------------------------------------------------------------
 * * que_want: Do we want this queue entry?
 */

static int que_want(BQUE *entry, dbref ptarg, dbref otarg) {
  if ((ptarg != NOTHING) && (ptarg != Owner(entry->player)))
    return 0;
  if ((otarg != NOTHING) && (otarg != entry->player))
    return 0;
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * halt_que: Remove all queued commands from a certain player
 */

int halt_que(dbref player, dbref object) {
  BQUE *trail, *point, *next;
  OBJQE *pque;

  int numhalted;

  numhalted = 0;

  /* Player's que */
  // XXX: nuke queu

  pque = cque_find(player);
  if (pque && pque->cque) {
    while ((point = cque_deque(player)) != NULL) {
      free(point->text);
      point->text = NULL;
      free_qentry(point);
      point = NULL;
      numhalted++;
    }
  }
  pque = cque_find(object);
  if (pque && pque->cque) {
    while ((point = cque_deque(object)) != NULL) {
      free(point->text);
      point->text = NULL;
      free_qentry(point);
      point = NULL;
      numhalted++;
    }
  }

  /*
   * Wait queue
   */

  for (point = mudstate.qwait, trail = NULL; point; point = next)
    if (que_want(point, player, object)) {
      numhalted++;
      if (trail)
        trail->next = next = point->next;
      else
        mudstate.qwait = next = point->next;
      if (evtimer_pending(&point->ev, NULL))
        evtimer_del(&point->ev);
      free(point->text);
      free_qentry(point);
    } else
      next = (trail = point)->next;

  /*
   * Semaphore queue
   */

  for (point = mudstate.qsemfirst, trail = NULL; point; point = next)
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
      free_qentry(point);
    } else
      next = (trail = point)->next;

  if (player == NOTHING)
    player = Owner(object);
  giveto(player, (mudconf.waitcost * numhalted));
  if (object == NOTHING)
    s_Queue(player, 0);
  else
    a_Queue(player, -numhalted);
  return numhalted;
}

/*
 * ---------------------------------------------------------------------------
 * * do_halt: Command interface to halt_que.
 */

void do_halt(dbref player, dbref cause, int key, char *target) {
  dbref player_targ, obj_targ;
  int numhalted;

  if ((key & HALT_ALL) && !(Can_Halt(player))) {
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
      player_targ = Owner(player);
      if (Typeof(player) != TYPE_PLAYER)
        obj_targ = player;
    }
  } else {
    if (Can_Halt(player))
      obj_targ = match_thing(player, target);
    else
      obj_targ = match_controlled(player, target);

    if (obj_targ == NOTHING)
      return;
    if (key & HALT_ALL) {
      notify(player, "Can't specify a target and /all");
      return;
    }
    if (Typeof(obj_targ) == TYPE_PLAYER) {
      player_targ = obj_targ;
      obj_targ = NOTHING;
    } else {
      player_targ = NOTHING;
    }
  }

  numhalted = halt_que(player_targ, obj_targ);
  if (Quiet(player))
    return;
  if (numhalted == 1)
    notify(Owner(player), "1 queue entries removed.");
  else
    notify_printf(Owner(player), "%d queue entries removed.", numhalted);
}

/*
 * ---------------------------------------------------------------------------
 * * nfy_que: Notify commands from the queue and perform or discard them.
 */

int nfy_que(dbref sem, int attr, int key, int count) {
  BQUE *point, *trail, *next;
  int num;
  long aflags;
  dbref aowner;
  char *str;

  if (attr) {
    str = atr_get(sem, attr, &aowner, &aflags);
    num = atoi(str);
    free_lbuf(str);
  } else {
    num = 1;
  }

  if (num > 0) {
    num = 0;
    for (point = mudstate.qsemfirst, trail = NULL; point; point = next) {
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
          giveto(point->player, mudconf.waitcost);
          a_Queue(Owner(point->player), -1);
          free(point->text);
          free_qentry(point);
        }
      } else {
        next = (trail = point)->next;
      }

      /*
       * If we've notified enough, exit
       */

      if ((key == NFY_NFY) && (num >= count))
        next = NULL;
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
    atr_clr(sem, attr);

  return num;
}

/*
 * ---------------------------------------------------------------------------
 * * do_notify: Command interface to nfy_que
 */

void do_notify(dbref player, dbref cause, int key, char *what, char *count) {
  dbref thing, aowner;
  int loccount, attr = -1;
  long aflags;
  ATTR *ap;
  char *obj;

  obj = parse_to(&what, '/', 0);
  init_match(player, obj, NOTYPE);
  match_everything(0);

  if ((thing = noisy_match_result()) < 0) {
    notify(player, "No match.");
  } else if (!controls(player, thing)) {
    notify(player, "Permission denied.");
  } else {
    if (!what || !*what) {
      ap = NULL;
    } else {
      ap = atr_str(what);
    }

    if (!ap) {
      attr = A_SEMAPHORE;
    } else {
      /* Do they have permission to set this attribute? */
      atr_pget_info(thing, ap->number, &aowner, &aflags);
      if (Set_attr(player, thing, ap, aflags)) {
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
      if (!(Quiet(player) || Quiet(thing))) {
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

static BQUE *setup_que(dbref player, dbref cause, char *command, char *args[],
                       int nargs, char *sargs[]) {
  int a, tlen;
  BQUE *tmp;
  char *tptr;

  /*
   * Can we run commands at all?
   */

  if (Halted(player))
    return NULL;

  /*
   * make sure player can afford to do it
   */

  a = mudconf.waitcost;
  if (mudconf.machinecost && ((random() % mudconf.machinecost) == 0))
    a++;
  if (!payfor(player, a)) {
    notify(Owner(player), "Not enough money to queue command.");
    return NULL;
  }
  /*
   * Wizards and their objs may queue up to db_top+1 cmds. Players are
   * * * * * * * limited to QUEUE_QUOTA. -mnp
   */

  a = QueueMax(Owner(player));
  if (a_Queue(Owner(player), 1) > a) {
    notify(Owner(player),
           "Run away objects: too many commands queued.  Halted.");
    halt_que(Owner(player), NOTHING);

    /*
     * halt also means no command execution allowed
     */
    s_Halted(player);
    return NULL;
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
  tmp->comm = NULL;
  for (a = 0; a < NUM_ENV_VARS; a++) {
    tmp->env[a] = NULL;
  }
  for (a = 0; a < MAX_GLOBAL_REGS; a++) {
    tmp->scr[a] = NULL;
  }

  tptr = tmp->text = (char *)malloc(tlen);
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

  evtimer_set(&tmp->ev, wakeup_wait_que, tmp);

  tmp->player = player;
  tmp->waittime = 0;
  tmp->next = NULL;
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

void wait_que(dbref player, dbref cause, int wait, dbref sem, int attr,
              char *command, char *args[], int nargs, char *sargs[]) {
  BQUE *cmd;
  if (mudconf.control_flags & CF_INTERP)
    cmd = setup_que(player, cause, command, args, nargs, sargs);
  else
    cmd = NULL;

  if (cmd == NULL) {
    return;
  }

  if (wait > 0) {
    cmd->waittime = mudstate.now + wait;
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

void do_wait(dbref player, dbref cause, int key, char *event, char *cmd,
             char *cargs[], int ncargs) {
  dbref thing, aowner;
  int howlong, num, attr;
  long aflags;
  char *what;
  ATTR *ap;

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
  if (!Good_obj(thing)) {
    notify(player, "No match.");
  } else if (!controls(player, thing)) {
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
      ap = atr_str(event);
      if (!ap) {
        attr = mkattr(event);
        if (attr <= 0) {
          notify_quiet(player, "Invalid attribute.");
          return;
        }
        ap = atr_num(attr);
      }
      atr_pget_info(thing, ap->number, &aowner, &aflags);
      if (attr && Set_attr(player, thing, ap, aflags)) {
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
  char *cmdsave;

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
  mudstate.debug_cmd = (char *)"< do_second >";

  /*
   * Note: the point->waittime test would be 0 except the command is
   * being put in the low priority queue to be done in one
   * second anyways
   */

  /*
   * Check the semaphore queue for expired timed-waits
   */

  for (point = mudstate.qsemfirst, trail = NULL; point; point = next) {
    if (point->waittime == 0) {
      next = (trail = point)->next;
      continue; /*
                 * Skip if not timed-wait
                 */
    }
    if (point->waittime <= mudstate.now) {
      if (trail != NULL)
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
  dbref object;
  int count, i;
  char *command, *cp, *cmdsave;

  if ((mudconf.control_flags & CF_DEQUEUE) == 0)
    return 0;

  cmdsave = mudstate.debug_cmd;
  mudstate.debug_cmd = (char *)"< do_top >";

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
      if (mudstate.qhead == NULL)
        mudstate.qtail = NULL;
    } else {
      mudstate.qtail->next = mudstate.qhead;
      mudstate.qtail = mudstate.qtail->next;
      mudstate.qhead = mudstate.qhead->next;
      mudstate.qtail->next = NULL;
    }
    if (!tmp)
      continue;

    dassert(tmp);
    count++;
    if ((object >= 0) && !Going(object)) {
      giveto(object, mudconf.waitcost);
      mudstate.curr_enactor = tmp->cause;
      mudstate.curr_player = object;
      a_Queue(Owner(object), -1);
      if (!Halted(object)) {
        for (i = 0; i < MAX_GLOBAL_REGS; i++) {
          if (tmp->scr[i]) {
            StringCopy(mudstate.global_regs[i], tmp->scr[i]);
          } else {
            *mudstate.global_regs[i] = '\0';
          }
        }

        command = tmp->comm;

        if (command) {
          if (isPlayer(object) && Connected(object))
            choke_player(object);
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
                  mudstate.pout = NULL;
                }

                *mudstate.poutbufc = '\0';
                mudstate.pout = mudstate.poutnew;
                cp = parse_to(&command, ';', 0);
              }
              mudstate.inpipe = 0;
              process_command(object, tmp->cause, 0, cp, tmp->env, tmp->nargs);
              if (mudstate.pout) {
                free_lbuf(mudstate.pout);
                mudstate.pout = NULL;
              }
            }
          }
          if (isPlayer(object) && Connected(object))
            release_player(object);
        }
      }
    }
    free(tmp->text);
    free_qentry(tmp);
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

static void show_que(dbref player, int key, BQUE *queue, int *qent,
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
      if ((player != Owner(tmp->player)))
        continue;
    if ((tmp->waittime > 0) && (Good_obj(tmp->sem)))
      notify_printf(player, "[#%d/%d]%s:%s", tmp->sem,
                    tmp->waittime - mudstate.now, bufp, tmp->comm);
    else if (tmp->waittime > 0)
      notify_printf(player, "[%d]%s:%s", tmp->waittime - mudstate.now, bufp,
                    tmp->comm);
    else if (Good_obj(tmp->sem))
      notify_printf(player, "[#%d]%s:%s", tmp->sem, bufp, tmp->comm);
    else
      notify_printf(player, "%s:%s", bufp, tmp->comm);

    bp = bufp;
    if (key == PS_LONG) {
      for (i = 0; i < (tmp->nargs); i++) {
        if (tmp->env[i] != NULL) {
          safe_str((char *)"; Arg", bufp, &bp);
          safe_chr(i + '0', bufp, &bp);
          safe_str((char *)"='", bufp, &bp);
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

void do_ps(dbref player, dbref cause, int key, char *target) {
  dbref player_targ, obj_targ;
  int pqent, pqtot, wqent, wqtot, sqent, sqtot;
  OBJQE *objq;
  int tempkey;

  /*
   * Figure out what to list the queue for
   */

  if ((key & PS_ALL) && !(See_Queue(player))) {
    notify(player, "Permission denied.");
    return;
  }
  if (!target || !*target) {
    obj_targ = NOTHING;
    if (key & PS_ALL) {
      player_targ = NOTHING;
    } else {
      player_targ = Owner(player);
      if (Typeof(player) != TYPE_PLAYER)
        obj_targ = player;
    }
  } else {
    player_targ = Owner(player);
    obj_targ = match_controlled(player, target);
    if (obj_targ == NOTHING)
      return;
    if (key & PS_ALL) {
      notify(player, "Can't specify a target and /all");
      return;
    }
    if (Typeof(obj_targ) == TYPE_PLAYER) {
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
    while (objq && (objq = objq->next) != NULL) {
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

  if (See_Queue(player))
    notify_printf(player,
                  "Totals: Player...%d/%d  Wait...%d/%d  Semaphore...%d/%d",
                  pqent, pqtot, wqent, wqtot, sqent, sqtot);
  else
    notify_printf(player,
                  "Totals: Player...%d/%d  Wait...%d/%d  Semaphore...%d/%d",
                  pqent, pqtot, wqent, wqtot, sqent, sqtot);
}

/*
 * ---------------------------------------------------------------------------
 * * do_queue: Queue management
 */

void do_queue(dbref player, dbref cause, int key, char *arg) {
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
    if (!Quiet(player))
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
    if (Quiet(player))
      return;
    if (i > 0)
      notify_printf(player, "WaitQ timer advanced %d seconds.", i);
    else if (i < 0)
      notify_printf(player, "WaitQ timer set back %d seconds.", i);
    else
      notify(player, "Object queue appended to player queue.");
  }
}
