
/* Schedules and executes timed MUX events. */

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
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "mux/network/mux_event.h"
#include "mux/server/event_timer.h"
#include "mux/support/checked_storage.h"

void mux_event_scheduler_initialize(MuxEventScheduler *scheduler) {
  memset(scheduler, 0, sizeof(*scheduler));
  scheduler->last_type = -1;
}

void mux_event_scheduler_set_loop(MuxEventScheduler *scheduler, UvLoopT *loop) {
  scheduler->loop = loop;
}

static void mux_event_release_owned_data(int flags, void *data,
                                         MuxEventPayload secondary) {
  if (flags & FLAG_FREE_DATA)
    free(data);
  if (flags & FLAG_FREE_SECONDARY) {
    assert(secondary.kind == MUX_EVENT_PAYLOAD_POINTER);
    if (secondary.kind == MUX_EVENT_PAYLOAD_POINTER &&
        (!(flags & FLAG_FREE_DATA) || secondary.pointer != data))
      free(secondary.pointer);
  }
}

void mux_event_scheduler_destroy(MuxEventScheduler *scheduler) {
  MuxEvent *event;
  MuxEvent *next;

  if (scheduler == nullptr)
    return;
  for (event = scheduler->events; event != nullptr; event = next) {
    next = event->next_in_main;
    mux_timer_destroy(event->timer);
    event->timer = nullptr;
    mux_event_release_owned_data(event->flags, event->data, event->secondary);
    free(event);
  }
  for (event = scheduler->free_events; event != nullptr; event = next) {
    next = event->next;
    free(event);
  }
  free((void *)scheduler->first_by_type);
  mux_event_scheduler_initialize(scheduler);
}

/* The main add-to-lists event handling function */

static void mux_event_delete(MuxEvent * /*e*/);

static MuxEvent **mux_event_type_slot(MuxEventScheduler *scheduler, int type) {
  if (type < 0 || type > scheduler->last_type)
    abort();
  return (MuxEvent **)checked_storage_at(
      (void *)scheduler->first_by_type, (size_t)scheduler->last_type + 1,
      sizeof(*scheduler->first_by_type), (size_t)type);
}

static MuxEvent *mux_event_type_head(MuxEventScheduler *scheduler, int type) {
  return *mux_event_type_slot(scheduler, type);
}

static void mux_event_main_list_add(MuxEventScheduler *scheduler, MuxEvent *e) {
  MuxEvent *old_head = scheduler->events;

  e->next_in_main = old_head;
  if (old_head)
    old_head->prev_in_main = e;
  scheduler->events = e;
  e->prev_in_main = nullptr;
}

static void mux_event_main_list_remove(MuxEvent *e) {
  MuxEventScheduler *scheduler = e->scheduler;
  if (e->prev_in_main)
    e->prev_in_main->next_in_main = e->next_in_main;
  if (e->next_in_main)
    e->next_in_main->prev_in_main = e->prev_in_main;
  if (scheduler->events == e) {
    scheduler->events = e->next_in_main;
    if (scheduler->events)
      scheduler->events->prev_in_main = nullptr;
  }
}

static void mux_event_type_list_add(MuxEventScheduler *scheduler, int type,
                                    MuxEvent *e) {
  MuxEvent **head = mux_event_type_slot(scheduler, type);
  MuxEvent *old_head = *head;

  e->next_in_type = old_head;
  if (old_head)
    old_head->prev_in_type = e;
  *head = e;
  e->prev_in_type = nullptr;
}

static void mux_event_type_list_remove(MuxEvent *e) {
  MuxEventScheduler *scheduler = e->scheduler;
  int type = (int)e->type;

  if (e->prev_in_type)
    e->prev_in_type->next_in_type = e->next_in_type;
  if (e->next_in_type)
    e->next_in_type->prev_in_type = e->prev_in_type;
  MuxEvent **head = mux_event_type_slot(scheduler, type);
  if (*head == e) {
    *head = e->next_in_type;
    if (*head)
      (*head)->prev_in_type = nullptr;
  }
}

#define is_zombie(e) ((e)->flags & FLAG_ZOMBIE)
#define LOOP_TYPE(type, var)                                                   \
  for ((var) = mux_event_type_head(scheduler, type); var;                      \
       (var) = (var)->next_in_type)                                            \
    if (!is_zombie(var))

#define LOOP_EVENT(var)                                                        \
  for ((var) = scheduler->events; var; (var) = (var)->next_in_main)            \
    if (!is_zombie(var))

static void mux_event_wakeup(MuxTimer *timer [[maybe_unused]], void *arg) {
  MuxEvent *e = (MuxEvent *)arg;

  if (is_zombie(e)) {
    mux_event_delete(e);
    return;
  }
  e->function(e);
  mux_event_delete(e);
}

void mux_event_add(const MuxEventRequest *request) {
  MuxEventScheduler *scheduler = request->scheduler;
  int time = request->delay;
  int flags = request->flags;
  int type = request->type;
  MuxEventCallback func = request->callback;
  void *data = request->data;
  MuxEvent *e = (MuxEvent *)0xDEADBEEF;
  int i;

  if (type < 0) {
    mux_event_release_owned_data(flags, data, request->secondary);
    return;
  }
  if (time < 1)
    time = 1;
  /* Event type heads grow with the highest registered type. */
  if (type > scheduler->last_type) {
    int previous_last_type = scheduler->last_type;
    MuxEvent **heads = (MuxEvent **)checked_storage_try_reallocate_array(
        (void *)scheduler->first_by_type, (size_t)type + 1, sizeof(*heads));
    if (heads == nullptr) {
      mux_event_release_owned_data(flags, data, request->secondary);
      return;
    }
    scheduler->first_by_type = heads;
    scheduler->last_type = type;
    for (i = previous_last_type + 1; i <= type; i++)
      *mux_event_type_slot(scheduler, i) = nullptr;
  }
  if (scheduler->free_events) {
    e = scheduler->free_events;
    scheduler->free_events = scheduler->free_events->next;
  } else {
    e = checked_storage_try_allocate(sizeof(MuxEvent));
    if (e == nullptr) {
      mux_event_release_owned_data(flags, data, request->secondary);
      return;
    }
    memset(e, 0, sizeof(MuxEvent));
  }

  e->flags = (char)flags;
  e->function = func;
  e->data = data;
  e->secondary = request->secondary;
  e->type = (char)type;
  e->tick = scheduler->tick + time;
  e->scheduler = scheduler;
  e->next = nullptr;

  e->timer = mux_timer_create(scheduler->loop, mux_event_wakeup, e);
  if (e->timer == nullptr) {
    mux_event_release_owned_data(flags, data, request->secondary);
    free(e);
    return;
  }
  if (!mux_timer_start(e->timer, (uint64_t)time * 1000, 0)) {
    mux_timer_destroy(e->timer);
    mux_event_release_owned_data(flags, data, request->secondary);
    free(e);
    return;
  }

  mux_event_main_list_add(scheduler, e);
  mux_event_type_list_add(scheduler, type, e);
}

/* Remove event */

static void mux_event_delete(MuxEvent *e) {
  MuxEventScheduler *scheduler = e->scheduler;
  mux_timer_destroy(e->timer);
  e->timer = nullptr;

  mux_event_release_owned_data(e->flags, e->data, e->secondary);

  mux_event_main_list_remove(e);
  mux_event_type_list_remove(e);

  e->next = scheduler->free_events;
  scheduler->free_events = e;
}

/* Run the thingy */

void mux_event_run(MuxEventScheduler *scheduler) { scheduler->tick += 1; }

int mux_event_run_by_type(MuxEventScheduler *scheduler, int type) {
  MuxEvent *e;
  int ran = 0;

  if (type >= 0 && type <= scheduler->last_type) {
    for (e = mux_event_type_head(scheduler, type); e; e = e->next_in_type) {
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

void mux_event_initialize(MuxEventScheduler *scheduler) { (void)scheduler; }

/* Event removal functions */

void mux_event_remove_data(MuxEventScheduler *scheduler, void *data) {
  MuxEvent *e;

  for (e = scheduler->events; e; e = e->next_in_main)
    if (e->data == data)
      e->flags |= FLAG_ZOMBIE;
}

void mux_event_remove_type_data(MuxEventScheduler *scheduler, int type,
                                void *data) {
  MuxEvent *e;

  if (type > scheduler->last_type)
    return;
  for (e = mux_event_type_head(scheduler, type); e; e = e->next_in_type)
    if (e->data == data) {

      e->flags |= FLAG_ZOMBIE;
    }
}

void mux_event_remove_type_secondary_pointer(MuxEventScheduler *scheduler,
                                             int type, const void *pointer) {
  MuxEvent *e;

  if (type > scheduler->last_type)
    return;
  for (e = mux_event_type_head(scheduler, type); e; e = e->next_in_type)
    if (e->secondary.kind == MUX_EVENT_PAYLOAD_POINTER &&
        e->secondary.pointer == pointer)
      e->flags |= FLAG_ZOMBIE;
}

void mux_event_remove_type_data_integer(MuxEventScheduler *scheduler, int type,
                                        const void *data, intptr_t integer) {
  MuxEvent *e;

  if (type > scheduler->last_type)
    return;
  for (e = mux_event_type_head(scheduler, type); e; e = e->next_in_type)
    if (e->data == data && e->secondary.kind == MUX_EVENT_PAYLOAD_INTEGER &&
        e->secondary.integer == integer)
      e->flags |= FLAG_ZOMBIE;
}

/* return the args of the event */
void mux_event_get_type_data(MuxEventScheduler *scheduler, int type,
                             const void *data, intptr_t *secondary_integer) {
  MuxEvent *e;

  LOOP_TYPE(type, e)
  if (e->data == data && e->secondary.kind == MUX_EVENT_PAYLOAD_INTEGER)
    *secondary_integer = e->secondary.integer;
}

/* All the counting / other kinds of 'useless' functions */
int mux_event_count_type(MuxEventScheduler *scheduler, int type) {
  MuxEvent *e;
  int count = 0;

  if (type > scheduler->last_type)
    return count;
  LOOP_TYPE(type, e) count++;
  return count;
}

int mux_event_count_type_data(MuxEventScheduler *scheduler, int type,
                              const void *data) {
  MuxEvent *e;
  int count = 0;

  if (type > scheduler->last_type)
    return count;
  LOOP_TYPE(type, e) if (e->data == data) count++;
  return count;
}

// The public query API consistently accepts the event type before its matcher.
// NOLINTBEGIN(bugprone-easily-swappable-parameters)
int mux_event_count_type_secondary_integer(MuxEventScheduler *scheduler,
                                           int type, intptr_t integer) {
  MuxEvent *e;
  int count = 0;

  if (type > scheduler->last_type)
    return count;
  LOOP_TYPE(type, e)
  if (e->secondary.kind == MUX_EVENT_PAYLOAD_INTEGER &&
      e->secondary.integer == integer)
    count++;
  return count;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

int mux_event_count_type_data_integer(MuxEventScheduler *scheduler, int type,
                                      const void *data, intptr_t integer) {
  MuxEvent *e;
  int count = 0;

  if (type > scheduler->last_type)
    return count;
  LOOP_TYPE(type, e)
  if (e->data == data && e->secondary.kind == MUX_EVENT_PAYLOAD_INTEGER &&
      e->secondary.integer == integer)
    count++;
  return count;
}

int mux_event_count_data(MuxEventScheduler *scheduler,
                         int type [[maybe_unused]], void *data) {
  MuxEvent *e;
  int count = 0;

  LOOP_EVENT(e) if (e->data == data) count++;
  return count;
}

void mux_event_gothru_type_data(MuxEventScheduler *scheduler, int type,
                                void *data, void (*func)(MuxEvent *)) {
  MuxEvent *e;

  if (type > scheduler->last_type)
    return;
  LOOP_TYPE(type, e) if (e->data == data) func(e);
}

void mux_event_visit_type_data(MuxEventScheduler *scheduler, int type,
                               void *data, void (*visitor)(MuxEvent *, void *),
                               void *context) {
  MuxEvent *event;

  if (type > scheduler->last_type)
    return;
  LOOP_TYPE(type, event) if (event->data == data) visitor(event, context);
}

void mux_event_visit_type(MuxEventScheduler *scheduler, int type,
                          void (*visitor)(MuxEvent *, void *), void *context) {
  MuxEvent *event;

  if (type > scheduler->last_type)
    return;
  LOOP_TYPE(type, event) visitor(event, context);
}

void mux_event_gothru_type(MuxEventScheduler *scheduler, int type,
                           void (*func)(MuxEvent *)) {
  MuxEvent *e;

  if (type > scheduler->last_type)
    return;
  LOOP_TYPE(type, e) func(e);
}

int mux_event_last_type_data(MuxEventScheduler *scheduler, int type,
                             const void *data) {
  MuxEvent *e;
  int last = 0;
  int t;

  if (type > scheduler->last_type)
    return last;
  for (e = mux_event_type_head(scheduler, type); e; e = e->next_in_type) {
    if (is_zombie(e) || e->data != data)
      continue;
    t = e->tick - scheduler->tick;
    if (t > last)
      last = t;
  }
  return last;
}

long mux_event_count_type_data_firstev(MuxEventScheduler *scheduler, int type,
                                       const void *data) {
  MuxEvent *e;

  if (type > scheduler->last_type)
    return -1;
  LOOP_TYPE(type, e)
  if (e->data == data && e->secondary.kind == MUX_EVENT_PAYLOAD_INTEGER) {
    return (long)e->secondary.integer;
  }
  return -1;
}
