/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "btech_channel.h"
#include "btech_event.h"
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "command_handlers_api.h"
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
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define _MECH_SENSOR_C
#include "autopilot.h"
#include "btechstats_api.h"
#include "mech_ecm_api.h"
#include "mech_events.h"
#include "mech_lite_api.h"
#include "mech_los_api.h"
#include "mech_sensor.h"
#include "mech_sensor_api.h"
#include "mech_tag_api.h"
#include "mech_utils_api.h"

const SensorDefinition *mech_sensor_definition(int sensor) {
  if (sensor < 0)
    abort();
  return checked_storage_at_const(sensors, NUM_SENSORS, sizeof(*sensors),
                                  (size_t)sensor);
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

int mech_sensor_to_hit_bonus(Mech *mech, Mech *target, int flag, int maplight,
                             float range, int ammunition_mode) {
  int bth1, bth2;
  int wLightMod = (ammunition_mode & AC_INCENDIARY_MODE) ? 1 : 0;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  maplight = maplight + wLightMod;

  if (maplight < 0)
    maplight = 0;

  if (maplight > 2)
    maplight = 2;

  if (!(flag & (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY)))
    return 10000;
  if (!(flag & BATTLE_MAP_LOS_SEEN_PRIMARY)) {
    bth2 = 1 + mech_sensor_definition(mech_sensor_index(mech, 1))
                   ->to_hit_bonus(mech, target, map, flag, maplight);
#ifdef SENSOR_BTH_DEBUG
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("%d: BTH S+%d", mech_dbref(mech), bth2));
#endif
    return bth2;
  }
  if (!(flag & BATTLE_MAP_LOS_SEEN_SECONDARY) ||
      (mech_sensor_index(mech, 0) == mech_sensor_index(mech, 1))) {
    bth1 = mech_sensor_definition(mech_sensor_index(mech, 0))
               ->to_hit_bonus(mech, target, map, flag, maplight);
#ifdef SENSOR_BTH_DEBUG
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("%d: BTH P+%d", mech_dbref(mech), bth1));
#endif
    return bth1;
  }
  bth1 = mech_sensor_definition(mech_sensor_index(mech, 0))
             ->to_hit_bonus(mech, target, map, flag, maplight);
  bth2 = 1 + mech_sensor_definition(mech_sensor_index(mech, 1))
                 ->to_hit_bonus(mech, target, map, flag, maplight);
#ifdef SENSOR_BTH_DEBUG
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("%d: BTH +%d/+%d", mech_dbref(mech), bth1, bth2));
#endif
  return MIN(bth1, bth2);
}

int mech_sensor_can_see(Mech *mech, Mech *target, int *flag, int arc,
                        float range, int mapvis, int maplight, int cloudbase) {
  int i, j = 0, sn;
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

      if (!mech_sensor_definition(sn)->can_see(mech, target, map, range, *flag))
        continue;

      if (!mech_sensor_definition(sn)->see_chance(target, map, sn, range,
                                                  mapvis, maplight))
        continue;

      if (mech_sensor_definition(sn)->maximum_variation &&
          mech_sensor_range_as_float(
              mech_sensor_minimum_variable_range(mech, sn)) < range)
        if (mech_sensor_range_as_float(
                mech_sensor_minimum_variable_range(mech, sn) +
                (mech_sensor_visibility_modifier(mech) *
                 (mech_sensor_definition(sn)->maximum_variation + 1)) /
                    100) < range)
          continue;
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
  if (!mech_sensor_definition(sn)->can_see(mech, target, map, range, *flag))
    return 0;
  if (!mech_sensor_definition(sn)->see_chance(target, map, sn, range, mapvis,
                                              maplight))
    return 0;
  if (mech_sensor_definition(sn)->maximum_variation &&
      mech_sensor_range_as_float(mech_sensor_minimum_variable_range(mech, sn)) <
          range)
    if (mech_sensor_range_as_float(
            mech_sensor_minimum_variable_range(mech, sn) +
            mech_sensor_visibility_modifier(mech) *
                (mech_sensor_definition(sn)->maximum_variation + 1) / 100) <
        range)
      return 0;
  return 3;
}

int mech_sensor_arc_base_chance(int type, int arc) {
  int base = 100;

  switch (type) {
  case CLASS_MW:
    if (arc & (LSIDEARC | RSIDEARC | REARARC))
      return 0;
    break;
  case CLASS_BSUIT:
  case CLASS_MECH:
    if (arc & (LSIDEARC | RSIDEARC))
      base = 70;
    else if (arc & REARARC)
      base = 40;
    break;
  default:
    if (arc & (LSIDEARC | RSIDEARC))
      base = 80;
    else if (arc & REARARC)
      base = 50;
    if (arc & TURRETARC)
      base += 15;
    break;
  }
  return base;
}

/* Slow, but sacrifices we make for sake of playability.. :-) */
int mech_sensor_driver_base_chance(Mech *mech) {
  int i = 1;
  int perception = mech_perception_target(mech);

  if (perception <= 2)
    i = 36;
  else if (perception >= 12)
    i = 1;
  else {
    static const int modifiers[] = {3, 6, 10, 15, 21, 26, 30, 33, 35};
    const int modifier = *(const int *)checked_storage_at_const(
        modifiers, sizeof(modifiers) / sizeof(*modifiers), sizeof(*modifiers),
        (size_t)(perception - 3));
    i = 36 - modifier;
  }
  return 64 + i; /* Padded a bit */
}

int mech_sensor_detects(Mech *mech, Mech *target, int f, int arc, float range,
                        int snum, int chance_divisor, int mapvis,
                        int maplight) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int chance = (mech_sensor_arc_base_chance(mech_class(mech), arc) *
                ((target && mech_team(mech) != mech_team(target))
                     ? mech_sensor_driver_base_chance(mech)
                     : 100)) /
               100;
  int ch2 = mech_sensor_definition(snum)->see_chance(target, map, snum, range,
                                                     mapvis, maplight);

  if (!ch2 ||
      !mech_sensor_definition(snum)->can_see(mech, target, map, range, f))
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
                       ((chance * ch2) / chance_divisor)) {
    if (target &&
        is_in_character(btech_context_database(mech_context(mech)),
                        mech_dbref(mech)) &&
        is_in_character(btech_context_database(mech_context(mech)),
                        mech_dbref(target)) &&
        mech_team(mech) != mech_team(target))
      if (!btech_random_range(mech_context(mech), 0, 5))
        MadePerceptionRoll(mech, -2);
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
int mech_sensor_detects_now(Mech *mech, Mech *target, int f, int arc,
                            float range, int mapvis, int maplight) {
  int i, sn;

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
      if (mech_sensor_detects(mech, target, f, arc, range, sn, i + 1, mapvis,
                              maplight))
        return (i + 1);
    }
    return 0;
  }
  sn = mech_sensor_index(mech, 0);
  if (mech_sensor_range_as_float(mech_sensor_maximum_range(mech, sn)) < range)
    return 0;
  return mech_sensor_detects(mech, target, f, arc, range, sn, 1, mapvis,
                             maplight);
}

SensorFlagText sensor_flag_text(int flags) {
  SensorFlagText text = {0};
  char *buffer = text.text;
  int j;

  for (j = 0; j < 32; j++)
    if (flags & (1 << j)) {
      if (buffer[0] == '\0')
        snprintf(buffer, sizeof(text.text), "%d", j);
      else {
        const size_t used = strlen(buffer);
        snprintf(checked_storage_region(buffer, sizeof(text.text), used,
                                        sizeof(text.text) - used),
                 sizeof(text.text) - used, ",%d", j);
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

unsigned short mech_sensor_visibility_update(Mech *mech, unsigned short flags,
                                             float range, int x, int y,
                                             Mech *target, int mapvis,
                                             int maplight, int cloudbase,
                                             int seeanew, int wlf) {
  int arc;
  float x1, y1;
  int sc, sl, st;
  int f = flags;
  char buf[MBUF_SIZE] = {0};

  if (!mech_is_started(mech))
    return flags;
  if (target) {
    x1 = mech_position_real_x(target);
    y1 = mech_position_real_y(target);
    if (mech_position_z(mech) >= ATMO_Z && mech_position_z(target) >= ATMO_Z)
      range = range / 3;
  } else
    MapCoordToRealCoord(x, y, &x1, &y1);
  arc = InWeaponArc(mech, x1, y1);
  if (f & BATTLE_MAP_LOS_SEEN) {
    if ((sl = mech_sensor_can_see(mech, target, &f, arc, range, mapvis,
                                  maplight, cloudbase))) {
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
           mech_autocon_include_shutdown_targets(mech)) &&
          (st = valid_to_notice(mech, target, wlf)) && seeanew < 3) {
        if (st & AUTOCON_WARN)
          strcpy(buf, "[fg=yellow]");
        else
          buf[0] = 0;
        if (st & AUTOCON_SHORT) {
          const size_t used = strlen(buf);
          snprintf(checked_storage_region(buf, sizeof(buf), used,
                                          sizeof(buf) - used),
                   sizeof(buf) - used, "Lost: %s, %s arc.",
                   mech_to_mech_display_id_base(mech, target, wlf).text,
                   GetArcID(mech, arc));
        } else
          snprintf(buf, sizeof(buf),
                   "You have lost %s from your scanners. It was last in your "
                   "%s arc.",
                   mech_to_mech_display_id_base(mech, target, wlf).text,
                   GetArcID(mech, arc));
        if (st & AUTOCON_WARN)
          strcat(buf, "[reset]");
        mech_notify(mech, MECHALL, buf);
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
  if ((sc = mech_sensor_can_see(mech, target, &f, arc, range, mapvis, maplight,
                                cloudbase))) {
    if (!seeanew) {
      mech_possible_contact_count_increment(mech);
      return (unsigned short)f;
    }
    if ((sl = mech_sensor_detects_now(mech, target, f, arc, range, mapvis,
                                      maplight))) {
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
           mech_autocon_include_shutdown_targets(mech)) &&
          (st = valid_to_notice(mech, target, -1)) && seeanew < 2) {
        if (st & AUTOCON_WARN)
          strcpy(buf, "[fg=red]");
        else
          buf[0] = 0;
        if (st & AUTOCON_SHORT) {
          const size_t used = strlen(buf);
          snprintf(checked_storage_region(buf, sizeof(buf), used,
                                          sizeof(buf) - used),
                   sizeof(buf) - used, "Seen: %s, %s arc.",
                   mech_to_mech_display_id(mech, target).text,
                   GetArcID(mech, arc));
        } else {
          const size_t used = strlen(buf);
          snprintf(checked_storage_region(buf, sizeof(buf), used,
                                          sizeof(buf) - used),
                   sizeof(buf) - used, "You notice %s in your %s arc.",
                   mech_to_mech_display_id(mech, target).text,
                   GetArcID(mech, arc));
        }
        if (st & AUTOCON_WARN)
          strcat(buf, "[reset]");
        mech_notify(mech, MECHALL, buf);
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
    } else
      mech_possible_contact_count_increment(mech);
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
  const int maximum_visibility = battle_map_maximum_visibility(map);
  if (range > mech_sensor_range_as_float(maximum_visibility) &&
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
  flags = (unsigned short)mech_los_calculate_flags(
      observer, target, map, mech_position_x(target), mech_position_y(target),
      flags, range);
  battle_map_los_flags_set(map, observer_index, target_index, flags);

#ifdef ADVANCED_LOS
  (void)always_update_sensors;
  flags = mech_sensor_visibility_update(
      observer, flags, range, -1, -1, target, battle_map_visibility(map),
      battle_map_light(map), battle_map_cloud_base(map), 0, was_visible);
  battle_map_los_flags_set(map, observer_index, target_index, flags);
#else
  if (always_update_sensors) {
    flags = mech_sensor_visibility_update(
        observer, flags, range, -1, -1, target, battle_map_visibility(map),
        battle_map_light(map), battle_map_cloud_base(map), 0, was_visible);
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

void mech_sensor_description_append(char *buf, int size, Mech *mech, int sn,
                                    int verbose) {
  if (size <= 0)
    return;
  const size_t capacity = (size_t)size;
  size_t used = strlen(buf);
  if (used >= capacity)
    return;
  if (!verbose)
    snprintf(checked_storage_region(buf, capacity, used, capacity - used),
             capacity - used, "(R:%s)",
             mech_sensor_definition(sn)->range_description);
  else {
    snprintf(checked_storage_region(buf, capacity, used, capacity - used),
             capacity - used, "\n\tRange:      %s\n\tBlocked by: %s",
             mech_sensor_definition(sn)->range_description,
             mech_sensor_definition(sn)->block_description);
    used = strlen(buf);
    if (mech_sensor_definition(sn)->special_description && used < capacity)
      snprintf(checked_storage_region(buf, capacity, used, capacity - used),
               capacity - used, "\n\tNotes:      %s",
               mech_sensor_definition(sn)->special_description);
  }
}
