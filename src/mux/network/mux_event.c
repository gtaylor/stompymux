
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Created: Tue Aug 27 19:01:55 1996 fingon
 * Last modified: Tue Nov 10 16:21:43 1998 fingon
 *
 */

/* mux_event.c - Timed event scheduling and execution. */

/* Interface for creating pretty damn nasty timed events, with
   additional load balancing in the works.

   Description of the interface:

   void mux_event_add()

   Adds a new event to occur <time> ticks from now on, which calls
   function func with the present event as parameter, and with data as
   the data (also optional type can be supplied ; just makes deletion
   of stuff of particular type far faster, and allows nice statistics)

   void mux_event_initialize()
   Initializes the event system

   void mux_event_run()
   Runs one 'tick' of events (second, 1/10sec, whatever)

   int mux_event_count_type(int type)
   int mux_event_count_type_data(int type, void *data)
   int mux_event_count_data(void *data)
   Counts pending events (count_type is fast ; count_type_data relatively
   slow and count_data a dog)
   int mux_event_last_type()
   Returns # of the last type that has been used
   int mux_event_last_type_data(int type, void *data)
   Finds the event furthest in the future and returns the difference
   in seconds to present time (or actually in event ticks)

   void mux_event_gothru_type_data(int type, void *data, void (*func)(MuxEvent
   *)) Executes the function func for every object in tye first_in_type queue
   matching type, and/or data.


*/

/* NOTE:
   This approach turns _very_ costly, if you have regularly events
   further than LOOKAHEAD_STACK_SIZE in the future
   */

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <sys/types.h>
#include <time.h>

#include "mux/network/mux_event.h"
#include "mux/server/diagnostics.h"
#include "mux/server/event_timer.h"

void mux_event_scheduler_initialize(MuxEventScheduler *scheduler) {
  memset(scheduler, 0, sizeof(*scheduler));
  scheduler->last_type = -1;
}

void mux_event_scheduler_set_loop(MuxEventScheduler *scheduler,
                                  uv_loop_t *loop) {
  scheduler->loop = loop;
}

void mux_event_scheduler_destroy(MuxEventScheduler *scheduler) {
  MuxEvent *event;
  MuxEvent *next;

  if (scheduler == nullptr)
    return;
  for (event = scheduler->events; event != nullptr; event = next) {
    next = event->next_in_main;
    if (event->flags & FLAG_FREE_DATA)
      free(event->data);
    if (event->flags & FLAG_FREE_DATA2)
      free(event->data2);
    free(event);
  }
  for (event = scheduler->free_events; event != nullptr; event = next) {
    next = event->next;
    free(event);
  }
  free(scheduler->first_by_type);
  mux_event_scheduler_initialize(scheduler);
}

/* Stack of the events according to date */
#define mux_event_first_in_type (scheduler->first_by_type)

/* Whole list (dual linked) */
#define mux_event_list (scheduler->events)

/* List of 'free' events */
#define mux_event_free_list (scheduler->free_events)

#define last_muxevent_type (scheduler->last_type)
/* The main add-to-lists event handling function */

static void mux_event_delete(MuxEvent *);

static void mux_event_main_list_add(MuxEventScheduler *scheduler, MuxEvent *e) {
  MuxEvent *old_head = mux_event_list;

  e->next_in_main = old_head;
  if (old_head)
    old_head->prev_in_main = e;
  mux_event_list = e;
  e->prev_in_main = nullptr;
}

static void mux_event_main_list_remove(MuxEvent *e) {
  MuxEventScheduler *scheduler = e->scheduler;
  if (e->prev_in_main)
    e->prev_in_main->next_in_main = e->next_in_main;
  if (e->next_in_main)
    e->next_in_main->prev_in_main = e->prev_in_main;
  if (mux_event_list == e) {
    mux_event_list = e->next_in_main;
    if (mux_event_list)
      mux_event_list->prev_in_main = nullptr;
  }
}

static void mux_event_type_list_add(MuxEventScheduler *scheduler, int type,
                                    MuxEvent *e) {
  MuxEvent *old_head = mux_event_first_in_type[type];

  e->next_in_type = old_head;
  if (old_head)
    old_head->prev_in_type = e;
  mux_event_first_in_type[type] = e;
  e->prev_in_type = nullptr;
}

static void mux_event_type_list_remove(MuxEvent *e) {
  MuxEventScheduler *scheduler = e->scheduler;
  int type = (int)e->type;

  if (e->prev_in_type)
    e->prev_in_type->next_in_type = e->next_in_type;
  if (e->next_in_type)
    e->next_in_type->prev_in_type = e->prev_in_type;
  if (mux_event_first_in_type[type] == e) {
    mux_event_first_in_type[type] = e->next_in_type;
    if (mux_event_first_in_type[type])
      mux_event_first_in_type[type]->prev_in_type = nullptr;
  }
}

#define is_zombie(e) (e->flags & FLAG_ZOMBIE)
#define LoopType(type, var)                                                    \
  for (var = mux_event_first_in_type[type]; var; var = var->next_in_type)      \
    if (!is_zombie(var))

#define LoopEvent(var)                                                         \
  for (var = mux_event_list; var; var = var->next_in_main)                     \
    if (!is_zombie(var))

static void mux_event_wakeup(MuxTimer *timer, void *arg) {
  MuxEvent *e = (MuxEvent *)arg;

  if (is_zombie(e)) {
    mux_event_delete(e);
    return;
  }
  e->function(e);
  mux_event_delete(e);
}

void mux_event_add(MuxEventScheduler *scheduler, int time, int flags, int type,
                   void (*func)(MuxEvent *), void *data, void *data2) {
  MuxEvent *e = (MuxEvent *)0xDEADBEEF;
  int i;

  if (time < 1)
    time = 1;
  /* Nasty thing about the new system : we _do_ have to allocate
     mux_event_first_in_type dynamically. */
  if (type > last_muxevent_type) {
    mux_event_first_in_type = realloc(mux_event_first_in_type,
                                      sizeof(MuxEvent *) * (size_t)(type + 1));
    for (i = last_muxevent_type + 1; i <= type; i++)
      mux_event_first_in_type[i] = nullptr;
    last_muxevent_type = type;
  }
  if (mux_event_free_list) {
    e = mux_event_free_list;
    mux_event_free_list = mux_event_free_list->next;
  } else {
    e = malloc(sizeof(MuxEvent));
    memset(e, 0, sizeof(MuxEvent));
  }

  e->flags = (char)flags;
  e->function = func;
  e->data = data;
  e->data2 = data2;
  e->type = (char)type;
  e->tick = scheduler->tick + time;
  e->scheduler = scheduler;
  e->next = nullptr;

  e->timer = mux_timer_create(scheduler->loop, mux_event_wakeup, e);
  if (e->timer == nullptr) {
    free(e);
    return;
  }
  mux_timer_start(e->timer, (uint64_t)time * 1000, 0);

  mux_event_main_list_add(scheduler, e);
  mux_event_type_list_add(scheduler, type, e);
}

/* Remove event */

static void mux_event_delete(MuxEvent *e) {
  MuxEventScheduler *scheduler = e->scheduler;
  mux_timer_destroy(e->timer);
  e->timer = nullptr;

  if (e->flags & FLAG_FREE_DATA)
    free((void *)e->data);
  if (e->flags & FLAG_FREE_DATA2)
    free((void *)e->data2);

  mux_event_main_list_remove(e);
  mux_event_type_list_remove(e);

  e->next = mux_event_free_list;
  mux_event_free_list = e;
}

/* Run the thingy */

void mux_event_run(MuxEventScheduler *scheduler) { scheduler->tick += 1; }

int mux_event_run_by_type(MuxEventScheduler *scheduler, int type) {
  MuxEvent *e;
  int ran = 0;

  if (type <= last_muxevent_type) {
    for (e = mux_event_first_in_type[type]; e; e = e->next_in_type) {
      if (!is_zombie(e)) {
        e->function(e);
        e->flags |= FLAG_ZOMBIE;
        ran++;
      }
    }
  }
  return ran;
}

int mux_event_last_type(const MuxEventScheduler *scheduler) {
  return scheduler->last_type;
}

/* Initialize the events */

void mux_event_initialize(MuxEventScheduler *scheduler) {
  (void)scheduler;
  dprintk("muxevent initializing");
}

/* Event removal functions */

void mux_event_remove_data(MuxEventScheduler *scheduler, void *data) {
  MuxEvent *e;

  for (e = mux_event_list; e; e = e->next_in_main)
    if (e->data == data)
      e->flags |= FLAG_ZOMBIE;
}

void mux_event_remove_type_data(MuxEventScheduler *scheduler, int type,
                                void *data) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  for (e = mux_event_first_in_type[type]; e; e = e->next_in_type)
    if (e->data == data) {

      e->flags |= FLAG_ZOMBIE;
    }
}

void mux_event_remove_type_data2(MuxEventScheduler *scheduler, int type,
                                 void *data) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  for (e = mux_event_first_in_type[type]; e; e = e->next_in_type)
    if (e->data2 == data)
      e->flags |= FLAG_ZOMBIE;
}

void mux_event_remove_type_data_data(MuxEventScheduler *scheduler, int type,
                                     void *data, void *data2) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  for (e = mux_event_first_in_type[type]; e; e = e->next_in_type)
    if (e->data == data && e->data2 == data2)
      e->flags |= FLAG_ZOMBIE;
}

/* return the args of the event */
void mux_event_get_type_data(MuxEventScheduler *scheduler, int type, void *data,
                             long *data2) {
  MuxEvent *e;

  LoopType(type, e) if (e->data == data) *data2 = (long)e->data2;
}

/* All the counting / other kinds of 'useless' functions */
int mux_event_count_type(MuxEventScheduler *scheduler, int type) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) count++;
  return count;
}

int mux_event_count_type_data(MuxEventScheduler *scheduler, int type,
                              void *data) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) if (e->data == data) count++;
  return count;
}

int mux_event_count_type_data2(MuxEventScheduler *scheduler, int type,
                               void *data) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) if (e->data2 == data) count++;
  return count;
}

int mux_event_count_type_data_data(MuxEventScheduler *scheduler, int type,
                                   void *data, void *data2) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) if (e->data == data && e->data2 == data2) count++;
  return count;
}

int mux_event_count_data(MuxEventScheduler *scheduler, int type, void *data) {
  MuxEvent *e;
  int count = 0;

  LoopEvent(e) if (e->data == data) count++;
  return count;
}

void mux_event_gothru_type_data(MuxEventScheduler *scheduler, int type,
                                void *data, void (*func)(MuxEvent *)) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  LoopType(type, e) if (e->data == data) func(e);
}

void mux_event_visit_type_data(MuxEventScheduler *scheduler, int type,
                               void *data, void (*visitor)(MuxEvent *, void *),
                               void *context) {
  MuxEvent *event;

  if (type > last_muxevent_type)
    return;
  LoopType(type, event) if (event->data == data) visitor(event, context);
}

void mux_event_visit_type(MuxEventScheduler *scheduler, int type,
                          void (*visitor)(MuxEvent *, void *), void *context) {
  MuxEvent *event;

  if (type > last_muxevent_type)
    return;
  LoopType(type, event) visitor(event, context);
}

void mux_event_gothru_type(MuxEventScheduler *scheduler, int type,
                           void (*func)(MuxEvent *)) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  LoopType(type, e) func(e);
}

int mux_event_last_type_data(MuxEventScheduler *scheduler, int type,
                             void *data) {
  MuxEvent *e;
  int last = 0, t;

  if (type > last_muxevent_type)
    return last;
  LoopType(type, e) if (e->data == data) if ((t = (e->tick - scheduler->tick)) >
                                             last) last = t;
  return last;
}

long mux_event_count_type_data_firstev(MuxEventScheduler *scheduler, int type,
                                       void *data) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return -1;
  LoopType(type, e) if (e->data == data) { return (long)(e->data2); }
  return -1;
}
