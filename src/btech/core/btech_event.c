/* Typed scheduling helpers for BTech objects. */

#include "btech_event.h"

#include <stddef.h> // IWYU pragma: keep

#include "autopilot.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "map_obj_api.h" // IWYU pragma: keep
#include "mech_api_types.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_stagger.h"
#include "mech_targeting_api.h"
#include "mux/network/mux_event.h" // IWYU pragma: keep

void btech_context_event_schedule(BtechContext *context, void *object, int type,
                                  MuxEventCallback callback, int delay,
                                  intptr_t data) {
  btech_event_schedule(context->events, object, type, callback, delay, data);
}

void mech_event_schedule(Mech *mech, int type, MuxEventCallback callback,
                         int delay, intptr_t data) {
  if (mech_dbref(mech) > 0) {
    btech_event_schedule(mech_context(mech)->events, mech, type, callback,
                         delay, data);
  }
}

void autopilot_event_schedule(Autopilot *autopilot, int type,
                              MuxEventCallback callback, int delay,
                              intptr_t data) {
  btech_event_schedule(autopilot->xcode.context->events, autopilot, type,
                       callback, delay, data);
}

void map_event_schedule(BattleMap *map, int type, MuxEventCallback callback,
                        int delay, intptr_t data) {
  btech_event_schedule(map->xcode.context->events, map, type, callback, delay,
                       data);
}

int mech_event_count(const Mech *mech, int type) {
  return btech_event_count(mech_context(mech)->events, mech, type);
}

int mech_event_count_data(const Mech *mech, int type, intptr_t data) {
  return btech_event_count_data(mech_context(mech)->events, mech, type, data);
}

long mech_event_first_delay(const Mech *mech, int type) {
  return btech_event_first_delay(mech_context(mech)->events, mech, type);
}

int mech_event_last_delay(const Mech *mech, int type) {
  return btech_event_last_delay(mech_context(mech)->events, mech, type);
}

long mech_event_data(const Mech *mech, int type) {
  return btech_event_data(mech_context(mech)->events, mech, type);
}

void mech_event_cancel(Mech *mech, int type) {
  btech_event_cancel(mech_context(mech)->events, mech, type);
}

void mech_event_cancel_data(Mech *mech, int type, intptr_t data) {
  btech_event_cancel_data(mech_context(mech)->events, mech, type, data);
}

void mech_events_cancel_all(Mech *mech) {
  btech_events_cancel_all(mech_context(mech)->events, mech);
}

void mech_event_visit(Mech *mech, int type, MuxEventVisitor visitor,
                      void *context) {
  mux_event_visit_type_data(mech_context(mech)->events, type, mech, visitor,
                            context);
}

bool mech_dumping_type(const Mech *mech, intptr_t type) {
  return mech_event_count_data(mech, EVENT_DUMP, type) ||
         mech_event_count_data(mech, EVENT_DUMP, 0);
}

void mech_stun_crew(Mech *mech) {
  mech_event_schedule(mech, EVENT_UNSTUN_CREW, unstun_crew_event, 60, 0);
}

void mech_stop_digging(Mech *mech) {
  mech_event_cancel(mech, EVENT_DIG);
  mech_digging_clear(mech);
}

void mech_stop_lock(Mech *mech) {
  mech_event_cancel(mech, EVENT_LOCK);
  mech_targeting_lock_modes_clear(mech);
  mech_targeting_aim_reset(mech);
}

void mech_lose_lock(Mech *mech) {
  mech_stop_lock(mech);
  mech_targeting_target_clear(mech);
  if (mech_targeting_has_specific_aim(mech)) {
    mech_notify(mech, MECHALL, "Location-specific targeting powers down.");
    mech_targeting_aim_reset(mech);
  }
}

void mech_start_stagger_check(Mech *mech) {
  mech_event_schedule(mech, EVENT_CHECK_STAGGER, check_stagger_event, 5, 0);
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
                     "Starting stagger check for %ld.", mech_dbref(mech));
}

void mech_stop_stagger_check(Mech *mech) {
  mech_event_cancel(mech, EVENT_CHECK_STAGGER);
  mech_stagger_tracking_reset(mech);
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
                     "Stopping stagger check for %ld.", mech_dbref(mech));
}

bool mech_move_mode_locked(const Mech *mech) {
  return mech_movement_modes_locked(mech) ||
         (mech_event_count(mech, EVENT_MOVEMODE) && !mech_is_dodging(mech));
}
