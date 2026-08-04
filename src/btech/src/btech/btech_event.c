/* Typed scheduling helpers for BTech objects. */

#include "btech_event.h"

#include <stddef.h> // IWYU pragma: keep

#include "autopilot.h"
#include "btech_channel.h"
#include "btech_context.h"
#include "btmacros.h"
#include "map.terrain.h"
#include "mech.events.h"
#include "mech.h"
#include "mech.lifecycle.h"
#include "mech.notify.h"
#include "mux/network/mux_event.h" // IWYU pragma: keep
#include "p.map.obj.h"             // IWYU pragma: keep
#include "p.mech.events.h"
#include "p.mech.notify.h"

void mech_event_schedule(MECH *mech, int type, MuxEventCallback callback,
                         int delay, intptr_t data) {
  if (mech->mynum > 0) {
    btech_event_schedule(mech->xcode.context->events, mech, type, callback,
                         delay, data);
  }
}

void autopilot_event_schedule(AUTO *autopilot, int type,
                              MuxEventCallback callback, int delay,
                              intptr_t data) {
  btech_event_schedule(autopilot->xcode.context->events, autopilot, type,
                       callback, delay, data);
}

void map_event_schedule(MAP *map, int type, MuxEventCallback callback,
                        int delay, intptr_t data) {
  btech_event_schedule(map->xcode.context->events, map, type, callback, delay,
                       data);
}

int mech_event_count(const MECH *mech, int type) {
  return btech_event_count(mech->xcode.context->events, mech, type);
}

int mech_event_count_data(const MECH *mech, int type, intptr_t data) {
  return btech_event_count_data(mech->xcode.context->events, mech, type, data);
}

long mech_event_first_delay(const MECH *mech, int type) {
  return btech_event_first_delay(mech->xcode.context->events, mech, type);
}

long mech_event_data(const MECH *mech, int type) {
  return btech_event_data(mech->xcode.context->events, mech, type);
}

void mech_event_cancel(MECH *mech, int type) {
  btech_event_cancel(mech->xcode.context->events, mech, type);
}

void mech_event_cancel_data(MECH *mech, int type, intptr_t data) {
  btech_event_cancel_data(mech->xcode.context->events, mech, type, data);
}

void mech_events_cancel_all(MECH *mech) {
  btech_events_cancel_all(mech->xcode.context->events, mech);
}

bool mech_dumping_type(const MECH *mech, intptr_t type) {
  return mech_event_count_data(mech, EVENT_DUMP, type) ||
         mech_event_count_data(mech, EVENT_DUMP, 0);
}

void mech_stun_crew(MECH *mech) {
  mech_event_schedule(mech, EVENT_UNSTUN_CREW, unstun_crew_event, 60, 0);
}

void mech_stop_digging(MECH *mech) {
  mech_event_cancel(mech, EVENT_DIG);
  MechTankCritStatus(mech) &= ~DIGGING_IN;
}

void mech_stop_lock(MECH *mech) {
  mech_event_cancel(mech, EVENT_LOCK);
  MechStatus(mech) &= ~LOCK_MODES;
  MechAim(mech) = NUM_SECTIONS;
}

void mech_lose_lock(MECH *mech) {
  mech_stop_lock(mech);
  MechTarget(mech) = -1;
  MechTargX(mech) = -1;
  MechTargY(mech) = -1;
  if (MechAim(mech) != NUM_SECTIONS) {
    mech_notify(mech, MECHALL, "Location-specific targeting powers down.");
    MechAim(mech) = NUM_SECTIONS;
  }
}

void mech_start_stagger_check(MECH *mech) {
  mech_event_schedule(mech, EVENT_CHECK_STAGGER, check_stagger_event, 5, 0);
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                     "Starting stagger check for %ld.", mech->mynum);
}

void mech_stop_stagger_check(MECH *mech) {
  mech_event_cancel(mech, EVENT_CHECK_STAGGER);
  mech->rd.staggerDamage = 0;
  mech->rd.lastStaggerNotify = 0;
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG,
                     "Stopping stagger check for %ld.", mech->mynum);
}

bool mech_move_mode_locked(const MECH *mech) {
  return (MechStatus2(mech) & MOVE_MODES_LOCK) ||
         (mech_event_count(mech, EVENT_MOVEMODE) &&
          !(MechStatus2(mech) & DODGING));
}
