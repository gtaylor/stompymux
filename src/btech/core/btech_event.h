/* Typed scheduling helpers for BTech objects. */

#pragma once

#include <stdint.h>

#include "mech_events.h"
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

/* Marker callback persisted for scheduled work whose skill roll failed. */
void mech_event_failure_marker(MuxEvent *event);

void mech_event_schedule(Mech *mech, MechEventType type,
                         MuxEventCallback callback, int delay, intptr_t data);
/** Schedules a mech event carrying a borrowed pointer payload. */
void mech_event_schedule_pointer(Mech *mech, MechEventType type,
                                 MuxEventCallback callback, int delay,
                                 void *pointer);
/** Schedules a mech event that takes ownership of its pointer payload. */
void mech_event_schedule_owned_pointer(Mech *mech, MechEventType type,
                                       MuxEventCallback callback, int delay,
                                       void *pointer);
void autopilot_event_schedule(Autopilot *autopilot, MechEventType type,
                              MuxEventCallback callback, int delay,
                              intptr_t data);
void map_event_schedule(BattleMap *map, MechEventType type,
                        MuxEventCallback callback, int delay, intptr_t data);
/** Schedules a map event carrying a borrowed pointer payload. */
void map_event_schedule_pointer(BattleMap *map, MechEventType type,
                                MuxEventCallback callback, int delay,
                                void *pointer);
/** Schedules a map event that takes ownership of its pointer payload. */
void map_event_schedule_owned_pointer(BattleMap *map, MechEventType type,
                                      MuxEventCallback callback, int delay,
                                      void *pointer);
void btech_event_schedule(MuxEventScheduler *events, void *object, int type,
                          MuxEventCallback callback, int delay, intptr_t data);
/** Schedules a BTech event carrying a borrowed pointer payload. */
void btech_event_schedule_pointer(MuxEventScheduler *events, void *object,
                                  int type, MuxEventCallback callback,
                                  int delay, void *pointer);
/** Schedules an event that takes ownership of its pointer payload. */
void btech_event_schedule_owned_pointer(MuxEventScheduler *events, void *object,
                                        int type, MuxEventCallback callback,
                                        int delay, void *pointer);
void btech_context_event_schedule(BtechContext *context, void *object, int type,
                                  MuxEventCallback callback, int delay,
                                  intptr_t data);
void btech_context_owned_event_schedule(BtechContext *context, void *object,
                                        int type, MuxEventCallback callback,
                                        int delay, intptr_t data);
int btech_event_count(MuxEventScheduler *events, const void *object, int type);
int btech_event_count_data(MuxEventScheduler *events, const void *object,
                           int type, intptr_t data);
long btech_event_first_delay(MuxEventScheduler *events, const void *object,
                             int type);
int btech_event_last_delay(MuxEventScheduler *events, const void *object,
                           int type);
long btech_event_data(MuxEventScheduler *events, const void *object, int type);
void btech_event_cancel(MuxEventScheduler *events, void *object, int type);
void btech_event_cancel_data(MuxEventScheduler *events, void *object, int type,
                             intptr_t data);
void btech_events_cancel_all(MuxEventScheduler *events, void *object);

int mech_event_count(const Mech *mech, MechEventType type);
int mech_event_count_data(const Mech *mech, MechEventType type, intptr_t data);
long mech_event_first_delay(const Mech *mech, MechEventType type);
int mech_event_last_delay(const Mech *mech, MechEventType type);
long mech_event_data(const Mech *mech, MechEventType type);
void mech_event_cancel(Mech *mech, MechEventType type);
void mech_event_cancel_data(Mech *mech, MechEventType type, intptr_t data);
void mech_events_cancel_all(Mech *mech);
void mech_event_visit(Mech *mech, MechEventType type, MuxEventVisitor visitor,
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
