/** @file
 * Defines timed MUX event data structures and scheduler interfaces.
 */
#pragma once

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
typedef struct MuxTimer MuxTimer;
typedef void (*MuxEventCallback)(MuxEvent *event);
struct MuxEvent {
  char flags;
  MuxEventCallback function;
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

typedef struct uv_loop_s UvLoopT;
struct MuxEventScheduler {
  MuxEvent **first_by_type;
  MuxEvent *events;
  MuxEvent *free_events;
  int last_type;
  int tick;
  UvLoopT *loop;
};

/** Initializes mux event scheduler. @param[out] scheduler Event scheduler. */

void mux_event_scheduler_initialize(MuxEventScheduler *scheduler);
/** Sets loop on mux event scheduler. @param[in,out] scheduler Event scheduler.
 * @param[in,out] loop Event loop. */

void mux_event_scheduler_set_loop(MuxEventScheduler *scheduler, UvLoopT *loop);
/** Destroys mux event scheduler. @param[in,out] scheduler Event scheduler. */

void mux_event_scheduler_destroy(MuxEventScheduler *scheduler);
/* Macro for handling simple lists.
   Where it applies: a = main list, b = thing to be added, c = next
   field. Reused across several unrelated list types, so it stays a
   macro rather than being tied to one struct's field names. */
#define ADD_TO_LIST_HEAD(a, c, b)                                              \
  b->c = a;                                                                    \
  (a) = b

typedef struct MuxEventRequest {
  MuxEventScheduler *scheduler;
  int delay;
  int flags;
  int type;
  MuxEventCallback callback;
  void *data;
  void *secondary_data;
} MuxEventRequest;

/** Adds mux event. @param[in] request Request. */

void mux_event_add(const MuxEventRequest *request);
/** Executes mux event run. @param[in,out] scheduler Event scheduler. */

void mux_event_run(MuxEventScheduler *scheduler);
/** Executes mux event run by type. @param[in] scheduler Event scheduler.
 * @param[in] type Type. */

int mux_event_run_by_type(MuxEventScheduler *scheduler, int type);
/** Executes mux event last type. @param[in] scheduler Event scheduler. */

int mux_event_last_type(const MuxEventScheduler *scheduler);
/** Initializes mux event. @param[out] scheduler Event scheduler. */

void mux_event_initialize(MuxEventScheduler *scheduler);
/** Removes data from mux event. @param[in,out] scheduler Event scheduler.
 * @param[in,out] data Caller-provided data. */

void mux_event_remove_data(MuxEventScheduler *scheduler, void *data);
/** Removes type data from mux event. @param[in,out] scheduler Event scheduler.
 * @param[in] type Type. @param[in,out] data Caller-provided data. */

void mux_event_remove_type_data(MuxEventScheduler *scheduler, int type,
                                void *data);
/** Removes type data2 from mux event. @param[in,out] scheduler Event scheduler.
 * @param[in] type Type. @param[in,out] data Caller-provided data. */

void mux_event_remove_type_data2(MuxEventScheduler *scheduler, int type,
                                 void *data);
/** Removes type data data from mux event. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in,out] data Caller-provided data.
 * @param[in,out] data2 Data2. */

void mux_event_remove_type_data_data(MuxEventScheduler *scheduler, int type,
                                     void *data, void *data2);
/** Returns type data from mux event. @param[in,out] scheduler Event scheduler.
 * @param[in] type Type. @param[in] data Caller-provided data. @param[in,out]
 * data2 Data2. */

void mux_event_get_type_data(MuxEventScheduler *scheduler, int type,
                             const void *data, long *data2);
/** Executes mux event count type. @param[in] scheduler Event scheduler.
 * @param[in] type Type. */

int mux_event_count_type(MuxEventScheduler *scheduler, int type);
/** Executes mux event count type data. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in] data Caller-provided data. */

int mux_event_count_type_data(MuxEventScheduler *scheduler, int type,
                              const void *data);
/** Executes mux event count type data2. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in,out] data Caller-provided data. */

int mux_event_count_type_data2(MuxEventScheduler *scheduler, int type,
                               void *data);
/** Executes mux event count type data data. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in] data Caller-provided data.
 * @param[in] data2 Data2. */

int mux_event_count_type_data_data(MuxEventScheduler *scheduler, int type,
                                   const void *data, const void *data2);
/** Executes mux event count data. @param[in,out] scheduler Event scheduler.
 * @param[in] type Type. @param[in,out] data Caller-provided data. */

int mux_event_count_data(MuxEventScheduler *scheduler, int type, void *data);
/** Executes mux event gothru type data. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in,out] data Caller-provided data.
 * @param[in] func Func. */

void mux_event_gothru_type_data(MuxEventScheduler *scheduler, int type,
                                void *data, MuxEventCallback func);
/** Executes mux event visit type data. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in,out] data Caller-provided data.
 * @param[in,out] visitor Visitor. @param[in,out] context Operation context. */

void mux_event_visit_type_data(MuxEventScheduler *scheduler, int type,
                               void *data, void (*visitor)(MuxEvent *, void *),
                               void *context);
/** Executes mux event visit type. @param[in] scheduler Event scheduler.
 * @param[in] type Type. @param[in] visitor Visitor. @param[in] context
 * Operation context. */

void mux_event_visit_type(MuxEventScheduler *scheduler, int type,
                          void (*visitor)(MuxEvent *, void *), void *context);
/** Executes mux event gothru type. @param[in] scheduler Event scheduler.
 * @param[in] type Type. @param[in] func Func. */

void mux_event_gothru_type(MuxEventScheduler *scheduler, int type,
                           MuxEventCallback func);
/** Executes mux event last type data. @param[in,out] scheduler Event scheduler.
 * @param[in] type Type. @param[in] data Caller-provided data. */

int mux_event_last_type_data(MuxEventScheduler *scheduler, int type,
                             const void *data);
/** Executes mux event count type data firstev. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in] data Caller-provided data. */

long mux_event_count_type_data_firstev(MuxEventScheduler *scheduler, int type,
                                       const void *data);

/* Did I mention cproto is braindead? */
