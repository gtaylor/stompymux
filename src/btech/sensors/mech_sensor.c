/* Implements BattleTech sensor mechanics for unit sensor. */

#include "btech_event.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_los_api.h"
#include "map_los_types.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_network_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"

#include "autopilot_resume_api.h"
#include "btechstats_api.h"
#include "mech_ecm_api.h"
#include "mech_events.h"
#include "mech_lite_api.h"
#include "mech_los_api.h"
#include "mech_sensor.h"
#include "mech_sensor_api.h"
#include "mech_tag_api.h"
#include "mech_utils_api.h"

const SensorDefinition SENSOR_DEFINITIONS[] = {
    {"Vislight", "V", 0, 60, 0, vislight_see, vislight_csee, vislight_tohit, -1,
     -1, 0, 1, SENSOR_ATTR_NONE, "Visual",
     "Fire/Smoke/Obstacles, 3 pt woods, 5 underwater hexes",
     "Bad in night-fighting (BTH)"},
    {"Light-amplification", "L", 0, 60, 0, liteamp_see, liteamp_csee,
     liteamp_tohit, 0, 1, 0 - NS_TECH, 1, SENSOR_ATTR_NONE,
     "Visual (Dawn/Dusk), 2x Visual (Night)",
     "Fire/Smoke/Obstacles, 2 pt woods, any water",
     "Somewhat harder enemy detection (than vislight), bad in forests "
     "(BTH/range)"},
    {"Infrared", "I", 1, 15, 0, infrared_see, infrared_csee, infrared_tohit, -1,
     -1, 0 - NS_TECH, 1, SENSOR_ATTR_NONE, "15", "Fire/Obstacles, 6 pt woods",
     "Easy to hit 'hot' targets, not very efficient in forests (BTH)"},
    {"Electromagnetic", "E", 1, 24, 8, electrom_see, electrom_csee,
     electrom_tohit, -1, -1, 0 - NS_TECH, 1, SENSOR_ATTR_NONE, "16-24",
     "Mountains/Obstacles, 8 pt woods",
     "Easy to hit heavies, good in forests (BTH), overall unreliable (chances "
     "of detection/BTH)"},
    {"Seismic", "S", 1, 8, 4, seismic_see, seismic_csee, seismic_tohit, -1, -1,
     0 - NS_TECH, 1, SENSOR_ATTR_SEISMIC, "4-8", "Nothing",
     "Easier heavy and/or moving object detection (although overall hard to "
     "detect with), somewhat unreliable(BTH)"},
    {"Radar", "R", 1, 180, 0, radar_see, radar_csee, radar_tohit, -1, -1,
     AA_TECH, 1, SENSOR_ATTR_NONE, "<=180",
     "Obstacles, enemy elevation (Enemy Z >= 10, range: 180, Enemy Z < 10, "
     "range: varies)",
     "Premier anti-aircraft sensor, partially negates partial cover(BTH), "
     "doesn't see targets that are too low for detection"},
    {"Beagle ActiveProbe", "B", 1, 6, 0, bap_see, bap_csee, bap_tohit, -1, -1,
     BEAGLE_PROBE_TECH, 1, SENSOR_ATTR_NONE, "<=6", "Nothing (except range)",
     "Ultimate sensor in close-range detection (slightly varying BTH, but "
     "ignores partial/woods/water)"},
    {"Light Beagle ActiveProbe", "A", 1, 3, 0, bap_see, bap_csee, bap_tohit, -1,
     -1, LIGHT_BAP_TECH, 1, SENSOR_ATTR_NONE, "<=3", "Nothing (except range)",
     "Short range, but ultimate sensor in close-range detection (slightly "
     "varying BTH, but ignores partial/woods/water)"},
    {"Bloodhound ActiveProbe", "H", 1, 8, 0, blood_see, blood_csee, blood_tohit,
     -1, -1, BLOODHOUND_PROBE_TECH, 2, SENSOR_ATTR_NONE, "<=8",
     "Nothing (except range)",
     "Superior version of the Beagle Active Probe (slightly varying BTH, but "
     "ignores partial/woods/water)"}};

const SensorDefinition *mech_sensor_definition(int sensor) {
  if (sensor < 0)
    abort();
  return checked_storage_at_const(SENSOR_DEFINITIONS, NUM_SENSORS,
                                  sizeof(*SENSOR_DEFINITIONS), (size_t)sensor);
}

static int mech_sensor_maximum_range(const Mech *mech, int sensor) {
  int stationary_bonus =
      sensor >= 2 && mech_movement_type(mech) == MOVE_NONE ? 140 : 100;
  return mech_sensor_definition(sensor)->maximum_visibility * stationary_bonus /
         100;
}

static float mech_sensor_range_as_float(int range) { return (float)range; }

static int mech_sensor_minimum_variable_range(const Mech *mech, int sensor) {
  return mech_sensor_maximum_range(mech, sensor) -
         mech_sensor_definition(sensor)->maximum_variation;
}

static bool mech_sensor_sees_in_all_directions(const Mech *mech) {
  return mech_movement_type(mech) == MOVE_NONE ||
         mech_class(mech) == CLASS_BSUIT;
}

int mech_sensor_to_hit_bonus(const MechSensorToHitRequest *request) {
  Mech *mech = request->observer;
  Mech *target = request->target;
  int flag = request->los_flags;
  int maplight = request->map_light;
  int bth1;
  int bth2;
  int w_light_mod = (request->ammunition_mode & AC_INCENDIARY_MODE) ? 1 : 0;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  SensorToHitRequest sensor_request = {
      .observer = mech,
      .target = target,
      .map = map,
      .flags = flag,
  };

  maplight = maplight + w_light_mod;

  if (maplight < 0)
    maplight = 0;

  if (maplight > 2)
    maplight = 2;
  sensor_request.light = maplight;

  if (!(flag & (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY)))
    return 10000;
  if (!(flag & BATTLE_MAP_LOS_SEEN_PRIMARY)) {
    bth2 = 1 + mech_sensor_definition(mech_sensor_index(mech, 1))
                   ->to_hit_bonus(&sensor_request);
#ifdef SENSOR_BTH_DEBUG
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("%d: BTH S+%d", mech_dbref(mech), bth2));
#endif
    return bth2;
  }
  if (!(flag & BATTLE_MAP_LOS_SEEN_SECONDARY) ||
      (mech_sensor_index(mech, 0) == mech_sensor_index(mech, 1))) {
    bth1 = mech_sensor_definition(mech_sensor_index(mech, 0))
               ->to_hit_bonus(&sensor_request);
#ifdef SENSOR_BTH_DEBUG
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("%d: BTH P+%d", mech_dbref(mech), bth1));
#endif
    return bth1;
  }
  bth1 = mech_sensor_definition(mech_sensor_index(mech, 0))
             ->to_hit_bonus(&sensor_request);
  bth2 = 1 + mech_sensor_definition(mech_sensor_index(mech, 1))
                 ->to_hit_bonus(&sensor_request);
#ifdef SENSOR_BTH_DEBUG
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("%d: BTH +%d/+%d", mech_dbref(mech), bth1, bth2));
#endif
  return min(bth1, bth2);
}

int mech_sensor_can_see(const MechSensorObservationRequest *request) {
  Mech *mech = request->observer;
  Mech *target = request->target;
  int *flag = request->los_flags;
  int arc = request->arc;
  float range = request->range;
  int mapvis = request->map_visibility;
  int maplight = request->map_light;
  int cloudbase = request->cloud_base;
  int i;
  int j = 0;
  int sn;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  /* Make sure we're okay */
  if (!map || !mech)
    return 0;

  if (!(*flag & BATTLE_MAP_LOS_TERRAIN_CALCULATED))
    return 0;
  if (target && mech_is_invisible(target))
    return 0;
  /* Ok.. s'pose we can, at that. */
  if (mech_sensor_index(mech, 0) != mech_sensor_index(mech, 1)) {
    /* Check both seperately */
    for (i = 0; i < 2; i++) {
      sn = mech_sensor_index(mech, i);
      /* No chance */

      if (!mech_sensor_definition(sn)->full_vision &&
          !(arc & (FORWARDARC | TURRETARC)) &&
          !mech_sensor_sees_in_all_directions(mech))
        continue;
      if (mech_sensor_range_as_float(mech_sensor_maximum_range(mech, sn)) <
          range)
        continue;

      /* Okay, so this is a horrible, horrible hack that'll break when new
       * sensors get added. Thankfully it's not as ugly as the rest of the
       * constructs in this area, so i dont really care. -Foo */

      if (target) {
        if (cloudbase && sn < 3 &&
            ((mech_position_z(mech) < cloudbase)
                 ? (mech_position_z(target) >= cloudbase)
                 : (mech_position_z(target) < cloudbase)))
          continue;
      } else {
        if (cloudbase && mech_position_z(mech) > cloudbase)
          continue;
      }

      SensorContactRequest contact = {
          .observer = mech,
          .target = target,
          .map = map,
          .range = range,
          .flags = *flag,
      };
      if (!mech_sensor_definition(sn)->can_see(&contact))
        continue;

      SensorVisibilityRequest visibility = {
          .target = target,
          .map = map,
          .sensor = sn,
          .range = range,
          .condition_range = mapvis,
          .light = maplight,
      };
      if (!mech_sensor_definition(sn)->see_chance(&visibility))
        continue;

      if (mech_sensor_definition(sn)->maximum_variation &&
          mech_sensor_range_as_float(
              mech_sensor_minimum_variable_range(mech, sn)) < range) {
        if (mech_sensor_range_as_float(
                mech_sensor_minimum_variable_range(mech, sn) +
                ((mech_sensor_visibility_modifier(mech) *
                  (mech_sensor_definition(sn)->maximum_variation + 1)) /
                 100)) < range)
          continue;
      }
      j += (i + 1);
    }
    return j;
  }
  sn = mech_sensor_index(mech, 0);
  if (mech_sensor_range_as_float(mech_sensor_maximum_range(mech, sn)) < range)
    return 0;
  if (cloudbase && target && sn < 3 &&
      ((mech_position_z(mech) < cloudbase)
           ? (mech_position_z(target) >= cloudbase)
           : (mech_position_z(target) < cloudbase)))
    return 0;
  SensorContactRequest contact = {
      .observer = mech,
      .target = target,
      .map = map,
      .range = range,
      .flags = *flag,
  };
  if (!mech_sensor_definition(sn)->can_see(&contact))
    return 0;
  SensorVisibilityRequest visibility = {
      .target = target,
      .map = map,
      .sensor = sn,
      .range = range,
      .condition_range = mapvis,
      .light = maplight,
  };
  if (!mech_sensor_definition(sn)->see_chance(&visibility))
    return 0;
  if (mech_sensor_definition(sn)->maximum_variation &&
      mech_sensor_range_as_float(mech_sensor_minimum_variable_range(mech, sn)) <
          range) {
    if (mech_sensor_range_as_float(
            mech_sensor_minimum_variable_range(mech, sn) +
            (mech_sensor_visibility_modifier(mech) *
             (mech_sensor_definition(sn)->maximum_variation + 1) / 100)) <
        range)
      return 0;
  }
  return 3;
}

static int
mech_sensor_arc_base_chance(const MechSensorDetectionRequest *request) {
  int base = 100;
  int type = mech_class(request->observer);

  switch (type) {
  case CLASS_MW:
    if (request->arc & (LSIDEARC | RSIDEARC | REARARC))
      return 0;
    break;
  case CLASS_BSUIT:
  case CLASS_MECH:
    if (request->arc & (LSIDEARC | RSIDEARC))
      base = 70;
    else if (request->arc & REARARC)
      base = 40;
    break;
  default:
    if (request->arc & (LSIDEARC | RSIDEARC))
      base = 80;
    else if (request->arc & REARARC)
      base = 50;
    if (request->arc & TURRETARC)
      base += 15;
    break;
  }
  return base;
}

/* Slow, but sacrifices we make for sake of playability.. :-) */
int mech_sensor_driver_base_chance(Mech *mech) {
  int i = 1;
  int perception = mech_perception_target(mech);

  if (perception <= 2) {
    i = 36;
  } else if (perception >= 12) {
    i = 1;
  } else {
    static const int MODIFIERS[] = {3, 6, 10, 15, 21, 26, 30, 33, 35};
    const int MODIFIER = *(const int *)checked_storage_at_const(
        MODIFIERS, sizeof(MODIFIERS) / sizeof(*MODIFIERS), sizeof(*MODIFIERS),
        (size_t)(perception - 3));
    i = 36 - MODIFIER;
  }
  return 64 + i; /* Padded a bit */
}

int mech_sensor_detects(const MechSensorDetectionRequest *request) {
  Mech *mech = request->observer;
  Mech *target = request->target;
  int f = request->los_flags;
  float range = request->range;
  int snum = request->sensor;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int chance = (mech_sensor_arc_base_chance(request) *
                ((target && mech_team(mech) != mech_team(target))
                     ? mech_sensor_driver_base_chance(mech)
                     : 100)) /
               100;
  SensorVisibilityRequest visibility = {
      .target = target,
      .map = map,
      .sensor = snum,
      .range = range,
      .condition_range = request->map_visibility,
      .light = request->map_light,
  };
  SensorContactRequest contact = {
      .observer = mech,
      .target = target,
      .map = map,
      .range = range,
      .flags = f,
  };
  int ch2 = mech_sensor_definition(snum)->see_chance(&visibility);

  if (!ch2 || !mech_sensor_definition(snum)->can_see(&contact))
    return 0;
  if (target && mech_is_dropship(target))
    chance = chance * 4;
  if (target && mech_condition_summary(target).hidden &&
      mech_team(mech) != mech_team(target)) {

    if (mech_sensor_definition(snum)->match_letter[0] == 'B' &&
        mech_is_stealth_infantry(target) && !mech_is_purifier_infantry(target))
      return 0;

    if (ch2 <= 100) {
      if (range > 5)
        return 0;
      chance = chance / 4;
    }
  }
  if (range < 3 || btech_random_range(mech_context(mech), 1, 10000) <
                       ((chance * ch2) / request->chance_divisor)) {
    if (target &&
        is_in_character(btech_context_database(mech_context(mech)),
                        mech_dbref(mech)) &&
        is_in_character(btech_context_database(mech_context(mech)),
                        mech_dbref(target)) &&
        mech_team(mech) != mech_team(target))
      if (!btech_random_range(mech_context(mech), 0, 5))
        made_perception_roll(mech, -2);
    return 1;
  }
  return 0;
}

/* Main idea: If primary & secondary scanner differ,
   check both scanners for chance (with secondary at 1/2 chance).
   Also, if non-360degree scanners are used, check only forward arc for
   them.

   If both same, multiply chance by 1.1
 */
int mech_sensor_detects_now(const MechSensorDetectionRequest *request) {
  Mech *mech = request->observer;
  float range = request->range;
  int i;
  int sn;

  if (mech_sensor_index(mech, 0) != mech_sensor_index(mech, 1)) {
    /* Check both seperately */
    for (i = 0; i < 2; i++) {
      sn = mech_sensor_index(mech, i);
      /* No chance */
      if (!mech_sensor_definition(sn)->full_vision &&
          !(request->arc & (FORWARDARC | TURRETARC)) &&
          !mech_sensor_sees_in_all_directions(mech))
        continue;
      if (mech_sensor_range_as_float(mech_sensor_maximum_range(mech, sn)) <
          range)
        continue;
      MechSensorDetectionRequest sensor_request = *request;
      sensor_request.sensor = sn;
      sensor_request.chance_divisor = i + 1;
      if (mech_sensor_detects(&sensor_request))
        return (i + 1);
    }
    return 0;
  }
  sn = mech_sensor_index(mech, 0);
  if (mech_sensor_range_as_float(mech_sensor_maximum_range(mech, sn)) < range)
    return 0;
  MechSensorDetectionRequest sensor_request = *request;
  sensor_request.sensor = sn;
  sensor_request.chance_divisor = 1;
  return mech_sensor_detects(&sensor_request);
}

SensorFlagText sensor_flag_text(int flags) {
  SensorFlagText text = {0};
  char *buffer = text.text;
  int j;

  for (j = 0; j < 32; j++) {
    if (flags & (1 << j)) {
      if (buffer[0] == '\0') {
        (void)snprintf(buffer, sizeof(text.text), "%d", j);
      } else {
        const size_t USED = strlen(buffer);
        (void)snprintf(checked_storage_region(buffer, sizeof(text.text), USED,
                                              sizeof(text.text) - USED),
                       sizeof(text.text) - USED, ",%d", j);
      }
    }
  }
  return text;
}

#define AUTOCON_LONG 0x01
#define AUTOCON_WARN 0x02
#define AUTOCON_SHORT 0x04

static int valid_to_notice(Mech *mech, Mech *targ, int los) {
  int bf = (mech_brief_mode(mech) / 4);
  int foe;

  if ((los < 0 && mech_team(mech) == mech_team(targ) &&
       mech_los_check_unblocked(mech, targ, 0, 0, 0)) ||
      (los > 0 && mech_team(mech) == mech_team(targ)))
    foe = 0;
  else
    foe = AUTOCON_WARN;

  switch (bf) {
  case 0:
    return AUTOCON_LONG | foe;
  case 1:
    return AUTOCON_SHORT | foe;
  case 2:
    return foe ? (AUTOCON_LONG | foe) : 0;
  case 3:
    return foe ? (AUTOCON_SHORT | foe) : 0;
  case 4:
    return AUTOCON_SHORT;
  case 5:
    return foe ? AUTOCON_SHORT : 0;
  case 6:
  default:
    return 0;
  }
}

unsigned short
mech_sensor_visibility_update(const MechSensorVisibilityRequest *request) {
  Mech *mech = request->observer;
  unsigned short flags = request->los_flags;
  float range = request->range;
  Mech *target = request->target;
  int seeanew = request->notification_level;
  int wlf = request->previous_visibility;
  int arc;
  float x1;
  float y1;
  int sc;
  int sl;
  int st;
  int f = flags;
  char buf[MBUF_SIZE] = {0};

  if (!mech_is_started(mech))
    return flags;
  if (target) {
    x1 = mech_position_real_x(target);
    y1 = mech_position_real_y(target);
    if (mech_position_z(mech) >= ATMO_Z && mech_position_z(target) >= ATMO_Z)
      range = range / 3;
  } else {
    map_coord_to_real_coord(request->x, request->y, &x1, &y1);
  }
  arc = in_weapon_arc(mech, x1, y1);
  if (f & BATTLE_MAP_LOS_SEEN) {
    MechSensorObservationRequest observation = {
        .observer = mech,
        .target = target,
        .los_flags = &f,
        .arc = arc,
        .range = range,
        .map_visibility = request->map_visibility,
        .map_light = request->map_light,
        .cloud_base = request->cloud_base,
    };
    sl = mech_sensor_can_see(&observation);
    if (sl) {
      if (sl & 1)
        f |= BATTLE_MAP_LOS_SEEN_PRIMARY;
      if (sl & 2)
        f |= BATTLE_MAP_LOS_SEEN_SECONDARY;
    }
    if (!(f & (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY))) {
      if (mech_team(mech) != mech_team(target))
        mech_seen_count_decrement(mech);
      f &= ~BATTLE_MAP_LOS_SEEN;
      if (!mech_is_observer(mech) &&
          (mech_is_started(target) || mech_autocon_when_shutdown(mech) ||
           mech_autocon_include_shutdown_targets(mech))) {
        st = valid_to_notice(mech, target, wlf);
        if (st && seeanew < 3) {
          if (st & AUTOCON_WARN)
            strcpy(buf, "[fg=yellow]");
          else
            buf[0] = 0;
          if (st & AUTOCON_SHORT) {
            const size_t USED = strlen(buf);
            (void)snprintf(checked_storage_region(buf, sizeof(buf), USED,
                                                  sizeof(buf) - USED),
                           sizeof(buf) - USED, "Lost: %s, %s arc.",
                           mech_to_mech_display_id_base(mech, target, wlf).text,
                           get_arc_id(mech, arc));
          } else {
            (void)snprintf(
                buf, sizeof(buf),
                "You have lost %s from your scanners. It was last in your "
                "%s arc.",
                mech_to_mech_display_id_base(mech, target, wlf).text,
                get_arc_id(mech, arc));
          }
          if (st & AUTOCON_WARN)
            strlcat(buf, "[reset]", sizeof(buf));
          mech_notify(mech, MECHALL, buf);
        }
      }
      if (mech_targeting_has_lock_on(mech, mech_dbref(target))) {
        mech_notify(mech, MECHALL,
                    "Weapon system reports the lock has been lost.");
        mech_lose_lock(mech);
      }
#ifdef SENSOR_DEBUG
      snprintf(
          buf, strlen(buf), "Notice: #%d lost #%d (Sensor: %d, Flag: %s)",
          mech_dbref(mech), mech_dbref(target),
          (f & (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY)),
          sensor_flag_text(f).text);
      btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_SENSOR, "%s",
                         buf);
#endif
      mech_possible_contact_count_increment(mech);
    }
    return (unsigned short)f;
  }
  MechSensorObservationRequest observation = {
      .observer = mech,
      .target = target,
      .los_flags = &f,
      .arc = arc,
      .range = range,
      .map_visibility = request->map_visibility,
      .map_light = request->map_light,
      .cloud_base = request->cloud_base,
  };
  sc = mech_sensor_can_see(&observation);
  if (sc) {
    if (!seeanew) {
      mech_possible_contact_count_increment(mech);
      return (unsigned short)f;
    }
    MechSensorDetectionRequest detection = {
        .observer = mech,
        .target = target,
        .los_flags = f,
        .arc = arc,
        .range = range,
        .map_visibility = request->map_visibility,
        .map_light = request->map_light,
    };
    if (mech_sensor_detects_now(&detection)) {
      if (sc & 1)
        f |= BATTLE_MAP_LOS_SEEN_PRIMARY;
      if (sc & 2)
        f |= BATTLE_MAP_LOS_SEEN_SECONDARY;
    }
    if ((f & (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY))) {
      if (mech_team(mech) != mech_team(target)) {
        mech_seen_count_increment(mech);
        autopilot_resume_for_mech(mech);
      }
      f |= BATTLE_MAP_LOS_SEEN;
      if (!mech_is_observer(mech) &&
          (mech_is_started(target) || mech_autocon_when_shutdown(mech) ||
           mech_autocon_include_shutdown_targets(mech))) {
        st = valid_to_notice(mech, target, -1);
        if (st && seeanew < 2) {
          if (st & AUTOCON_WARN)
            strcpy(buf, "[fg=red]");
          else
            buf[0] = 0;
          if (st & AUTOCON_SHORT) {
            const size_t USED = strlen(buf);
            (void)snprintf(checked_storage_region(buf, sizeof(buf), USED,
                                                  sizeof(buf) - USED),
                           sizeof(buf) - USED, "Seen: %s, %s arc.",
                           mech_to_mech_display_id(mech, target).text,
                           get_arc_id(mech, arc));
          } else {
            const size_t USED = strlen(buf);
            (void)snprintf(checked_storage_region(buf, sizeof(buf), USED,
                                                  sizeof(buf) - USED),
                           sizeof(buf) - USED, "You notice %s in your %s arc.",
                           mech_to_mech_display_id(mech, target).text,
                           get_arc_id(mech, arc));
          }
          if (st & AUTOCON_WARN)
            strlcat(buf, "[reset]", sizeof(buf));
          mech_notify(mech, MECHALL, buf);
        }
      }
      if (mech_team(mech) != mech_team(target))
        mech_event_cancel(target, EVENT_HIDE);
#ifdef SENSOR_DEBUG
      snprintf(
          buf, sizeof(buf), "Notice: #%d saw #%d (Sensor: %d, Flag: %s C:%d)",
          mech_dbref(mech), mech_dbref(target),
          (f & (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY)),
          sensor_flag_text(f).text, seeanew);
      btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_SENSOR, "%s",
                         buf);
#endif
    } else {
      mech_possible_contact_count_increment(mech);
    }
  }
  return (unsigned short)f;
}

static void sensor_update_los_pair(BattleMap *map, int observer_index,
                                   int target_index, Mech *observer,
                                   Mech *target, bool always_update_sensors) {
  float range = mech_range_to(observer, target);

  if (mech_electronic_warfare_is_enabled(observer) && range < ECM_RANGE)
    mech_ecm_check(target);
  if (mech_tag_target_dbref(observer) > 0)
    mech_tag_check(observer);
  if (mech_searchlight_active(observer) && range < LITE_RANGE)
    cause_lite(observer, target);
  const int MAXIMUM_VISIBILITY = battle_map_maximum_visibility(map);
  if (range > mech_sensor_range_as_float(MAXIMUM_VISIBILITY) &&
      mech_position_z(target) < 11 && mech_position_z(observer) < 11) {
    battle_map_los_flags_set(map, observer_index, target_index,
                             BATTLE_MAP_LOS_BLOCKED);
    return;
  }

  unsigned short flags =
      battle_map_los_flags(map, observer_index, target_index);
  int was_visible =
      !(flags & BATTLE_MAP_LOS_BLOCKED) &&
      (flags & (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY));
  flags = (unsigned short)mech_los_calculate_flags(&(MechLosCalculation){
      .observer = observer,
      .target = target,
      .map = map,
      .target_hex = {.x = mech_position_x(target),
                     .y = mech_position_y(target)},
      .previous_flags = flags,
      .hex_range = range,
  });
  battle_map_los_flags_set(map, observer_index, target_index, flags);
  MechSensorVisibilityRequest visibility_request = {
      .observer = observer,
      .los_flags = flags,
      .range = range,
      .x = -1,
      .y = -1,
      .target = target,
      .map_visibility = battle_map_visibility(map),
      .map_light = battle_map_light(map),
      .cloud_base = battle_map_cloud_base(map),
      .notification_level = 0,
      .previous_visibility = was_visible,
  };

#ifdef ADVANCED_LOS
  (void)always_update_sensors;
  flags = mech_sensor_visibility_update(&visibility_request);
  battle_map_los_flags_set(map, observer_index, target_index, flags);
#else
  if (always_update_sensors) {
    flags = mech_sensor_visibility_update(&visibility_request);
    battle_map_los_flags_set(map, observer_index, target_index, flags);
  }
#endif
}

void mech_sensor_map_los_update(DbRef obj, BattleMap *map) {
  (void)obj;
  int unit_count = battle_map_unit_count(map);

  for (int i = 0; i < unit_count; i++) {
    Mech *observer = btech_context_get_mech(battle_map_context(map),
                                            battle_map_unit_dbref(map, i));
    if (!observer || !mech_is_started(observer))
      continue;
    for (int j = i + 1; j < unit_count; j++) {
      Mech *target = btech_context_get_mech(mech_context(observer),
                                            battle_map_unit_dbref(map, j));
      if (target)
        sensor_update_los_pair(map, i, j, observer, target, false);
    }
  }

  for (int i = 1; i < unit_count; i++) {
    Mech *observer = btech_context_get_mech(battle_map_context(map),
                                            battle_map_unit_dbref(map, i));
    if (!observer || !mech_is_started(observer))
      continue;
    for (int j = 0; j < i; j++) {
      Mech *target = btech_context_get_mech(mech_context(observer),
                                            battle_map_unit_dbref(map, j));
      if (target)
        sensor_update_los_pair(map, i, j, observer, target, true);
    }
  }
  battle_map_unit_moved_flags_clear(map);
}
