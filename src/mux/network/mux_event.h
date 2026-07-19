
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Tue Aug 27 19:02:00 1996 fingon
 * Last modified: Sat Jun  6 22:20:36 1998 fingon
 *
 */

/* mux_event.h - Timed event data structures and scheduler interfaces. */

#pragma once

#include "mux/server/event_timer.h"

/* EVENT_DEBUG adds some useful debugging information to the structure
   / allows more diverse set of error messages to be shown. However,
   for a run-time version it's practically useless. */

/* #undef EVENT_DEBUG */

enum : int {
  FLAG_FREE_DATA = 1,  /* Free the 1st data segment after execution */
  FLAG_FREE_DATA2 = 2, /* Free the 2nd data segment after execution */
  /* Exists there just because we're too lazy to search for it everywhere -
   * dud */
  FLAG_ZOMBIE = 4,
};

/* ZOMBIE events aren't moved during reschedule, they instead die then.
   Killing them outside event_run is kinda unhealthy, therefore we set things
   just ZOMBIE and delete if it's convenient for us. */

/* Main idea: Events are arranged as follows:
   - next 1-60sec (depending on present timing) each their own
     linked list
   - next hour with each min in the own linked list
   - next 60 hours with each hour in the own linked list
   - the rest in one huge 'stack', ordered according to time
     */

/* typedef unsigned char byte; */

typedef struct MuxEvent MuxEvent;
typedef struct MuxEventScheduler MuxEventScheduler;
struct MuxEvent {
  char flags;
  void (*function)(MuxEvent *);
  void *data;
  void *data2;
  int tick; /* The tick this baby was first scheduled to go off */
  char type;
  MuxEvent *next;
  MuxEvent *next_in_main;
  MuxEvent *prev_in_main;
  MuxEvent *prev_in_type;
  MuxEvent *next_in_type;
  MuxTimer *timer;
  MuxEventScheduler *scheduler;
};

typedef struct uv_loop_s uv_loop_t;
struct MuxEventScheduler {
  MuxEvent **first_by_type;
  MuxEvent *events;
  MuxEvent *free_events;
  int last_type;
  int tick;
  uv_loop_t *loop;
};

void mux_event_scheduler_initialize(MuxEventScheduler *scheduler);
void mux_event_scheduler_set_loop(MuxEventScheduler *scheduler,
                                  uv_loop_t *loop);
void mux_event_scheduler_destroy(MuxEventScheduler *scheduler);
/* Macro for handling simple lists.
   Where it applies: a = main list, b = thing to be added, c = next
   field. Reused across several unrelated list types, so it stays a
   macro rather than being tied to one struct's field names. */
#define ADD_TO_LIST_HEAD(a, c, b)                                              \
  b->c = a;                                                                    \
  a = b

void mux_event_add(MuxEventScheduler *scheduler, int time, int flags, int type,
                   void (*func)(MuxEvent *), void *data, void *data2);
void mux_event_run(MuxEventScheduler *scheduler);
int mux_event_run_by_type(MuxEventScheduler *scheduler, int type);
int mux_event_last_type(const MuxEventScheduler *scheduler);
void mux_event_initialize(MuxEventScheduler *scheduler);
void mux_event_remove_data(MuxEventScheduler *scheduler, void *data);
void mux_event_remove_type_data(MuxEventScheduler *scheduler, int type,
                                void *data);
void mux_event_remove_type_data2(MuxEventScheduler *scheduler, int type,
                                 void *data);
void mux_event_remove_type_data_data(MuxEventScheduler *scheduler, int type,
                                     void *data, void *data2);
void mux_event_get_type_data(MuxEventScheduler *scheduler, int type, void *data,
                             long *data2);
int mux_event_count_type(MuxEventScheduler *scheduler, int type);
int mux_event_count_type_data(MuxEventScheduler *scheduler, int type,
                              void *data);
int mux_event_count_type_data2(MuxEventScheduler *scheduler, int type,
                               void *data);
int mux_event_count_type_data_data(MuxEventScheduler *scheduler, int type,
                                   void *data, void *data2);
int mux_event_count_data(MuxEventScheduler *scheduler, int type, void *data);
void mux_event_gothru_type_data(MuxEventScheduler *scheduler, int type,
                                void *data, void (*func)(MuxEvent *));
void mux_event_visit_type_data(MuxEventScheduler *scheduler, int type,
                               void *data, void (*visitor)(MuxEvent *, void *),
                               void *context);
void mux_event_visit_type(MuxEventScheduler *scheduler, int type,
                          void (*visitor)(MuxEvent *, void *), void *context);
void mux_event_gothru_type(MuxEventScheduler *scheduler, int type,
                           void (*func)(MuxEvent *));
int mux_event_last_type_data(MuxEventScheduler *scheduler, int type,
                             void *data);
long mux_event_count_type_data_firstev(MuxEventScheduler *scheduler, int type,
                                       void *data);

/* Did I mention cproto is braindead? */
