#include "btech_channel.h"
#include "btech_event.h"
#include "map_conditions_api.h"
#include "map_los_api.h"
#include "map_units_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_lostracer_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

void mech_sensor_visibility_refresh(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int num = mech_map_slot(mech);

  if (!map)
    return;

  /* This is quiet; no message for noticing foes, etc. */
  /* This is a bonus effect in addition to movement-caused effects and is
     performed only once per move of the observed unit. */
  for (int i = 0; i < battle_map_unit_count(map); i++) {
    DbRef seer_dbref = battle_map_unit_dbref(map, i);
    if (i == num || seer_dbref < 0)
      continue;

    Mech *seer = btech_context_get_mech(mech_context(mech), seer_dbref);
    if (!seer)
      continue;
    if (mech_map_dbref(seer) != battle_map_dbref(map)) {
      btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("Mech #%ld was on map #%ld but with "
                                 "incorrect mapindex (%ld)",
                                 mech_dbref(seer), battle_map_dbref(map),
                                 mech_map_dbref(seer)));
      battle_map_unit_slot_clear(map, i);
      continue;
    }

    float range = mech_range_to(seer, mech);
    unsigned short los_flags = battle_map_los_flags(map, i, num);
    los_flags = (unsigned short)mech_los_calculate_flags(&(MechLosCalculation){
        .observer = seer,
        .target = mech,
        .map = map,
        .target_hex = {.x = mech_position_x(mech), .y = mech_position_y(mech)},
        .previous_flags = los_flags,
        .hex_range = range,
    });
    battle_map_los_flags_set(map, i, num, los_flags);

    /* Then update the SEES flags. */
    MechSensorVisibilityRequest request = {
        .observer = seer,
        .los_flags = los_flags,
        .range = range,
        .x = -1,
        .y = -1,
        .target = mech,
        .map_visibility = battle_map_visibility(map),
        .map_light = battle_map_light(map),
        .cloud_base = battle_map_cloud_base(map),
        .notification_level = 2,
    };
    los_flags = mech_sensor_visibility_update(&request);
    battle_map_los_flags_set(map, i, num, los_flags);
  }
}

static void mech_unblind_event(MuxEvent *event) {
  Mech *mech = event->data;

  mech_blinded_set(mech, false);
  if (!mech_pilot_is_unconscious(mech))
    mech_notify(mech, MECHALL, "Your sight recovers.");
}

void mech_sensors_scramble_infrared_and_liteamp(
    const SensorScrambleRequest *request) {
  Mech *mech = request->source;
  const int TIME = request->duration;
  const int CHANCE = request->chance;
  const char *inframsg = request->infrared_message;
  const char *liteampmsg = request->light_amplification_message;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  mech_sensor_visibility_refresh(mech);
  for (int i = 0; i < battle_map_unit_count(map); i++) {
    DbRef observer_dbref = battle_map_unit_dbref(map, i);
    if (observer_dbref == -1 || observer_dbref == mech_dbref(mech))
      continue;

    Mech *observer = btech_context_get_mech(mech_context(mech), observer_dbref);
    if (!observer ||
        !mech_los_check(observer, mech, mech_position_x(mech),
                        mech_position_y(mech), mech_range_to(observer, mech)))
      continue;
    if (mech_is_blinded(observer) || mech_pilot_is_unconscious(observer))
      continue;

    int sensor = mech_sensor_index(observer, 0);
    const char *match_letter = mech_sensor_definition(sensor)->match_letter;
    if (*match_letter == 'I' ||
        *checked_string_suffix(match_letter, 1) == 'I') {
      if (CHANCE && btech_random_range(mech_context(mech), 1, 100) > CHANCE)
        continue;
      mech_notify(observer, MECHALL, inframsg);
    } else if (*match_letter == 'L' ||
               *checked_string_suffix(match_letter, 1) == 'L') {
      if (CHANCE && btech_random_range(mech_context(mech), 1, 100) > CHANCE)
        continue;
      mech_notify(observer, MECHALL, liteampmsg);
    } else {
      continue;
    }

    mech_blinded_set(observer, true);
    mech_event_schedule(observer, EVENT_BLINDREC, mech_unblind_event, TIME, 0);
  }
}
