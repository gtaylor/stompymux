
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

typedef struct my_event_type {
  char flags;
  void (*function)(struct my_event_type *);
  void *data;
  void *data2;
  int tick; /* The tick this baby was first scheduled to go off */
  char type;
  struct my_event_type *next;
  struct my_event_type *next_in_main;
  struct my_event_type *prev_in_main;
  struct my_event_type *prev_in_type;
  struct my_event_type *next_in_type;
  MuxTimer *timer;
} MuxEvent;

/* Some external things _do_ use this one */
extern int mux_event_tick;
extern int events_scheduled;
extern int events_executed;
extern int events_zombies;

/* Macro for handling simple lists.
   Where it applies: a = main list, b = thing to be added, c = next
   field. Reused across several unrelated list types, so it stays a
   macro rather than being tied to one struct's field names. */
#define ADD_TO_LIST_HEAD(a, c, b)                                              \
  b->c = a;                                                                    \
  a = b

void mux_event_add(int time, int flags, int type, void (*func)(MuxEvent *),
                   void *data, void *data2);
void mux_event_run(void);
int mux_event_run_by_type(int type);
int mux_event_last_type(void);
void mux_event_initialize(void);
void mux_event_remove_data(void *data);
void mux_event_remove_type_data(int type, void *data);
void mux_event_remove_type_data2(int type, void *data);
void mux_event_remove_type_data_data(int type, void *data, void *data2);
void mux_event_get_type_data(int type, void *data, long *data2);
int mux_event_count_type(int type);
int mux_event_count_type_data(int type, void *data);
int mux_event_count_type_data2(int type, void *data);
int mux_event_count_type_data_data(int type, void *data, void *data2);
int mux_event_count_data(int type, void *data);
void mux_event_gothru_type_data(int type, void *data, void (*func)(MuxEvent *));
void mux_event_gothru_type(int type, void (*func)(MuxEvent *));
int mux_event_last_type_data(int type, void *data);
long mux_event_count_type_data_firstev(int type, void *data);

/* Did I mention cproto is braindead? */
