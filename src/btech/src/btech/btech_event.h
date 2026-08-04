/* Typed scheduling helpers for BTech objects. */

#pragma once

#include <stdint.h>

#include "mux/network/mux_event.h"

// IWYU pragma: no_include "map.h"

typedef struct MuxEvent MuxEvent;
typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct mech_data MECH;
typedef struct map_data MAP;
typedef struct auto_data AUTO;
typedef void (*MuxEventCallback)(MuxEvent *event);

void mech_event_schedule(MECH *mech, int type, MuxEventCallback callback,
                         int delay, intptr_t data);
void autopilot_event_schedule(AUTO *autopilot, int type,
                              MuxEventCallback callback, int delay,
                              intptr_t data);
void map_event_schedule(MAP *map, int type, MuxEventCallback callback,
                        int delay, intptr_t data);
void btech_event_schedule(MuxEventScheduler *events, void *object, int type,
                          MuxEventCallback callback, int delay, intptr_t data);
int btech_event_count(MuxEventScheduler *events, const void *object, int type);
int btech_event_count_data(MuxEventScheduler *events, const void *object,
                           int type, intptr_t data);
long btech_event_first_delay(MuxEventScheduler *events, const void *object,
                             int type);
long btech_event_data(MuxEventScheduler *events, const void *object, int type);
void btech_event_cancel(MuxEventScheduler *events, void *object, int type);
void btech_event_cancel_data(MuxEventScheduler *events, void *object, int type,
                             intptr_t data);
void btech_events_cancel_all(MuxEventScheduler *events, void *object);

int mech_event_count(const MECH *mech, int type);
int mech_event_count_data(const MECH *mech, int type, intptr_t data);
long mech_event_first_delay(const MECH *mech, int type);
long mech_event_data(const MECH *mech, int type);
void mech_event_cancel(MECH *mech, int type);
void mech_event_cancel_data(MECH *mech, int type, intptr_t data);
void mech_events_cancel_all(MECH *mech);
const char *btech_event_name(int type);
bool mech_dumping_type(const MECH *mech, intptr_t type);
void mech_stun_crew(MECH *mech);
void mech_stop_digging(MECH *mech);
void mech_stop_lock(MECH *mech);
void mech_lose_lock(MECH *mech);
void mech_start_stagger_check(MECH *mech);
void mech_stop_stagger_check(MECH *mech);
bool mech_move_mode_locked(const MECH *mech);
