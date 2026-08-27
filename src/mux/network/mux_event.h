/** @file
 * Defines timed MUX event data structures and scheduler interfaces.
 */
#pragma once

#include <stdint.h>

/* EVENT_DEBUG adds some useful debugging information to the structure
   / allows more diverse set of error messages to be shown. However,
   for a run-time version it's practically useless. */

/* #undef EVENT_DEBUG */

enum : int {
  FLAG_FREE_DATA = 1,      /* Free the 1st data segment after execution */
  FLAG_FREE_SECONDARY = 2, /* Free an owned secondary pointer payload */
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

/** Identifies the active member of a secondary event payload. */
typedef enum MuxEventPayloadKind : unsigned char {
  MUX_EVENT_PAYLOAD_NONE,
  MUX_EVENT_PAYLOAD_POINTER,
  MUX_EVENT_PAYLOAD_INTEGER,
} MuxEventPayloadKind;

/** Secondary event payload with explicit none/pointer/integer representation.
 */
typedef struct MuxEventPayload {
  MuxEventPayloadKind kind;
  union {
    void *pointer;
    intptr_t integer;
  };
} MuxEventPayload;

struct MuxEvent {
  char flags;
  MuxEventCallback function;
  void *data;
  MuxEventPayload secondary;
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
  MuxEventPayload secondary;
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
/** Marks events with a matching type and pointer payload for removal.
 * @param[in,out] scheduler Event scheduler. @param[in] type Event type.
 * @param[in] pointer Borrowed pointer payload to match. */

void mux_event_remove_type_secondary_pointer(MuxEventScheduler *scheduler,
                                             int type, const void *pointer);
/** Marks events with matching type, primary data, and integer payload.
 * @param[in,out] scheduler Event scheduler. @param[in] type Event type.
 * @param[in] data Primary event data. @param[in] integer Integer payload. */

void mux_event_remove_type_data_integer(MuxEventScheduler *scheduler, int type,
                                        const void *data, intptr_t integer);
/** Gets the last matching event's integer payload.
 * @param[in,out] scheduler Event scheduler. @param[in] type Event type.
 * @param[in] data Primary event data.
 * @param[out] secondary_integer Matching integer payload, if present. */

void mux_event_get_type_data(MuxEventScheduler *scheduler, int type,
                             const void *data, intptr_t *secondary_integer);
/** Executes mux event count type. @param[in] scheduler Event scheduler.
 * @param[in] type Type. */

int mux_event_count_type(MuxEventScheduler *scheduler, int type);
/** Executes mux event count type data. @param[in,out] scheduler Event
 * scheduler. @param[in] type Type. @param[in] data Caller-provided data. */

int mux_event_count_type_data(MuxEventScheduler *scheduler, int type,
                              const void *data);
/** Counts events with a matching type and integer payload.
 * @param[in] scheduler Event scheduler. @param[in] type Event type.
 * @param[in] integer Integer payload. */

int mux_event_count_type_secondary_integer(MuxEventScheduler *scheduler,
                                           int type, intptr_t integer);
/** Counts events with matching type, primary data, and integer payload.
 * @param[in] scheduler Event scheduler. @param[in] type Event type.
 * @param[in] data Primary event data. @param[in] integer Integer payload. */

int mux_event_count_type_data_integer(MuxEventScheduler *scheduler, int type,
                                      const void *data, intptr_t integer);
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
