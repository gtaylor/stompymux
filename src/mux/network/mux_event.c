
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

#include <event2/event.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <sys/types.h>
#include <time.h>

#include "mux/network/mux_event.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/server/debug.h"
#include "mux/server/server_lifecycle.h"

int mux_event_tick = 0;

/* Stack of the events according to date */
static MuxEvent **mux_event_first_in_type = nullptr;

/* Whole list (dual linked) */
static MuxEvent *mux_event_list = nullptr;

/* List of 'free' events */
static MuxEvent *mux_event_free_list = nullptr;

static int last_muxevent_type = -1;
/* The main add-to-lists event handling function */

extern void prerun_event(MuxEvent *e);
extern void postrun_event(MuxEvent *e);

static void mux_event_delete(MuxEvent *);

#define is_zombie(e) (e->flags & FLAG_ZOMBIE)
#define LoopType(type, var)                                                    \
  for (var = mux_event_first_in_type[type]; var; var = var->next_in_type)      \
    if (!is_zombie(var))

#define LoopEvent(var)                                                         \
  for (var = mux_event_list; var; var = var->next_in_main)                     \
    if (!is_zombie(var))

static void mux_event_wakeup(evutil_socket_t fd, short event, void *arg) {
  MuxEvent *e = (MuxEvent *)arg;

  if (is_zombie(e)) {
    mux_event_delete(e);
    return;
  }
  prerun_event(e);
  e->function(e);
  postrun_event(e);
  mux_event_delete(e);
}

void mux_event_add(int time, int flags, int type, void (*func)(MuxEvent *),
                   void *data, void *data2) {
  MuxEvent *e = (MuxEvent *)0xDEADBEEF;
  struct timeval tv = {0, 0};

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
  e->tick = mux_event_tick + time;
  e->next = nullptr;

  tv.tv_sec = time;
  tv.tv_usec = 0;

  e->ev = evtimer_new(server_lifecycle_event_base(), mux_event_wakeup, e);
  if (e->ev == nullptr) {
    free(e);
    return;
  }
  evtimer_add(e->ev, &tv);

  ADD_TO_BIDIR_LIST_HEAD(mux_event_list, prev_in_main, next_in_main, e);
  ADD_TO_BIDIR_LIST_HEAD(mux_event_first_in_type[type], prev_in_type,
                         next_in_type, e);
}

/* Remove event */

static void mux_event_delete(MuxEvent *e) {
  if (event_pending(e->ev, EV_TIMEOUT, nullptr))
    event_del(e->ev);
  event_free(e->ev);
  e->ev = nullptr;

  if (e->flags & FLAG_FREE_DATA)
    free((void *)e->data);
  if (e->flags & FLAG_FREE_DATA2)
    free((void *)e->data2);

  REMOVE_FROM_BIDIR_LIST(mux_event_list, prev_in_main, next_in_main, e);
  REMOVE_FROM_BIDIR_LIST(mux_event_first_in_type[(int)e->type], prev_in_type,
                         next_in_type, e);
  ADD_TO_LIST_HEAD(mux_event_free_list, next, e);
}

/* Run the thingy */

void mux_event_run() { mux_event_tick += 1; }

int mux_event_run_by_type(int type) {
  MuxEvent *e;
  int ran = 0;

  if (type <= last_muxevent_type) {
    for (e = mux_event_first_in_type[type]; e; e = e->next_in_type) {
      if (!is_zombie(e)) {
        prerun_event(e);
        e->function(e);
        postrun_event(e);
        e->flags |= FLAG_ZOMBIE;
        ran++;
      }
    }
  }
  return ran;
}

int mux_event_last_type() { return last_muxevent_type; }

/* Initialize the events */

void mux_event_initialize() { dprintk("muxevent initializing"); }

/* Event removal functions */

void mux_event_remove_data(void *data) {
  MuxEvent *e;

  for (e = mux_event_list; e; e = e->next_in_main)
    if (e->data == data)
      e->flags |= FLAG_ZOMBIE;
}

void mux_event_remove_type_data(int type, void *data) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  for (e = mux_event_first_in_type[type]; e; e = e->next_in_type)
    if (e->data == data) {

      e->flags |= FLAG_ZOMBIE;
    }
}

void mux_event_remove_type_data2(int type, void *data) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  for (e = mux_event_first_in_type[type]; e; e = e->next_in_type)
    if (e->data2 == data)
      e->flags |= FLAG_ZOMBIE;
}

void mux_event_remove_type_data_data(int type, void *data, void *data2) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  for (e = mux_event_first_in_type[type]; e; e = e->next_in_type)
    if (e->data == data && e->data2 == data2)
      e->flags |= FLAG_ZOMBIE;
}

/* return the args of the event */
void mux_event_get_type_data(int type, void *data, long *data2) {
  MuxEvent *e;

  LoopType(type, e) if (e->data == data) *data2 = (long)e->data2;
}

/* All the counting / other kinds of 'useless' functions */
int mux_event_count_type(int type) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) count++;
  return count;
}

int mux_event_count_type_data(int type, void *data) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) if (e->data == data) count++;
  return count;
}

int mux_event_count_type_data2(int type, void *data) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) if (e->data2 == data) count++;
  return count;
}

int mux_event_count_type_data_data(int type, void *data, void *data2) {
  MuxEvent *e;
  int count = 0;

  if (type > last_muxevent_type)
    return count;
  LoopType(type, e) if (e->data == data && e->data2 == data2) count++;
  return count;
}

int mux_event_count_data(int type, void *data) {
  MuxEvent *e;
  int count = 0;

  LoopEvent(e) if (e->data == data) count++;
  return count;
}

void mux_event_gothru_type_data(int type, void *data,
                                void (*func)(MuxEvent *)) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  LoopType(type, e) if (e->data == data) func(e);
}

void mux_event_gothru_type(int type, void (*func)(MuxEvent *)) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return;
  LoopType(type, e) func(e);
}

int mux_event_last_type_data(int type, void *data) {
  MuxEvent *e;
  int last = 0, t;

  if (type > last_muxevent_type)
    return last;
  LoopType(type, e) if (e->data == data) if ((t = (e->tick - mux_event_tick)) >
                                             last) last = t;
  return last;
}

long mux_event_count_type_data_firstev(int type, void *data) {
  MuxEvent *e;

  if (type > last_muxevent_type)
    return -1;
  LoopType(type, e) if (e->data == data) { return (long)(e->data2); }
  return -1;
}
