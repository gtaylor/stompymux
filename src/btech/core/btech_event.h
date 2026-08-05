/* Typed scheduling helpers for BTech objects. */

#pragma once

#include <stdint.h>

#include "mux/network/mux_event.h"

// IWYU pragma: no_include "map.h"

typedef struct MuxEvent MuxEvent;
typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct BtechContext BtechContext;
typedef struct Mech Mech;
typedef struct BattleMap BattleMap;
typedef struct Autopilot Autopilot;
typedef void (*MuxEventCallback)(MuxEvent *event);
typedef void (*MuxEventVisitor)(MuxEvent *event, void *context);

void mech_event_schedule(Mech *mech, int type, MuxEventCallback callback,
                         int delay, intptr_t data);
void autopilot_event_schedule(Autopilot *autopilot, int type,
                              MuxEventCallback callback, int delay,
                              intptr_t data);
void map_event_schedule(BattleMap *map, int type, MuxEventCallback callback,
                        int delay, intptr_t data);
void btech_event_schedule(MuxEventScheduler *events, void *object, int type,
                          MuxEventCallback callback, int delay, intptr_t data);
void btech_context_event_schedule(BtechContext *context, void *object, int type,
                                  MuxEventCallback callback, int delay,
                                  intptr_t data);
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

int mech_event_count(const Mech *mech, int type);
int mech_event_count_data(const Mech *mech, int type, intptr_t data);
long mech_event_first_delay(const Mech *mech, int type);
long mech_event_data(const Mech *mech, int type);
void mech_event_cancel(Mech *mech, int type);
void mech_event_cancel_data(Mech *mech, int type, intptr_t data);
void mech_events_cancel_all(Mech *mech);
void mech_event_visit(Mech *mech, int type, MuxEventVisitor visitor,
                      void *context);
const char *btech_event_name(int type);
bool mech_dumping_type(const Mech *mech, intptr_t type);
void mech_stun_crew(Mech *mech);
void mech_stop_digging(Mech *mech);
void mech_stop_lock(Mech *mech);
void mech_lose_lock(Mech *mech);
void mech_start_stagger_check(Mech *mech);
void mech_stop_stagger_check(Mech *mech);
bool mech_move_mode_locked(const Mech *mech);
