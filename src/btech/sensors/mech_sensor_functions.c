#include "mech_sensor.h"

#include "btech/context.h"
#include "map_conditions_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

#include <math.h>
#include <stdlib.h>

static bool mech_sensor_target_is_small(const Mech *target) {
  return target &&
         (mech_class(target) == CLASS_BSUIT || mech_class(target) == CLASS_MW);
}

static bool terrain_is_water(char terrain) {
  return terrain == BATTLE_TERRAIN_ICE || terrain == BATTLE_TERRAIN_WATER ||
         terrain == BATTLE_TERRAIN_BRIDGE;
}

static bool mech_sensor_is_lit(const Mech *mech) {
  MechConditionSummary condition = mech_condition_summary(mech);
  return condition.illuminated || mech_searchlight_active(mech);
}

static int mech_sensor_base_elevation(Mech *mech) {
  int elevation = mech_position_elevation_magnitude(mech);
  return terrain_is_water(mech_real_terrain_get(mech)) ? -elevation : elevation;
}

static int mech_sensor_elevation_above_surface(Mech *mech) {
  char terrain = mech_real_terrain_get(mech);
  int base = mech_sensor_base_elevation(mech);
  int upper = terrain == BATTLE_TERRAIN_ICE ? 0 : base;
  int lower =
      terrain == BATTLE_TERRAIN_BRIDGE ? bridge_w_elevation(mech) : base;
  int z = mech_position_z(mech);
  return z - (upper <= z ? upper : lower);
}

static int sensor_woods_count(Mech *target, int flags) {
  int woods = battle_map_los_wood_count(flags);
  if (mech_sensor_base_elevation(target) + 2 < mech_position_z(target))
    return woods;
  if (mech_real_terrain_get(target) == BATTLE_TERRAIN_LIGHT_FOREST)
    return woods + 1;
  if (mech_real_terrain_get(target) == BATTLE_TERRAIN_HEAVY_FOREST)
    return woods + 2;
  return woods;
}

static int sensor_partial_cover_modifier(const Mech *target, int flags,
                                         int base) {
  return flags & BATTLE_MAP_LOS_PARTIAL_COVER
             ? base + (mech_condition_summary(target).hull_down ? 2 : 0)
             : 0;
}

static int sensor_heat_modifier(float heat) {
#ifdef BT_SCALED_INFRARED
  return heat <= 0 ? 2 : heat > 50 ? -2 : heat > 35 ? -1 : heat > 20 ? 0 : 1;
#else
  return heat <= 7 ? 2 : heat <= 10 ? 1 : heat <= 15 ? 0 : heat <= 22 ? -1 : -2;
#endif
}

static int sensor_weight_modifier(int tonnage) {
  return tonnage > 65 ? -1 : tonnage > 35 ? 0 : 1;
}

static int sensor_movement_modifier(float speed) {
  return fabsf(speed) >= 10.75F ? 1 : 0;
}

int vislight_see(Mech *target, BattleMap *map, int sensor, float range,
                 int condition_range, int light) {
  (void)map;
  (void)sensor;
  int illuminated_multiplier =
      !light && target && mech_sensor_is_lit(target) ? 3 : 1;
  if (range > condition_range * illuminated_multiplier)
    return 0;
  return (int)((100 - (range / 3)) /
               (mech_sensor_target_is_small(target) ? 3 : 1));
}

int liteamp_see(Mech *target, BattleMap *map, int sensor, float range,
                int condition_range, int light) {
  (void)map;
  (void)sensor;
  if ((!light && range > 2 * condition_range) ||
      (light && range > condition_range))
    return 0;
  return (int)((70 - range) / (mech_sensor_target_is_small(target) ? 3 : 1));
}

int infrared_see(Mech *target, BattleMap *map, int sensor, float range,
                 int condition_range, int light) {
  (void)target;
  (void)map;
  (void)sensor;
  (void)condition_range;
  (void)light;
  return (int)(80 - range);
}

int electrom_see(Mech *target, BattleMap *map, int sensor, float range,
                 int condition_range, int light) {
  (void)target;
  (void)map;
  (void)sensor;
  (void)condition_range;
  (void)light;
  float close_range = range < 24 ? 2 : 0;
  float range_chance = 60 - range * 2;
  return (int)((close_range > range_chance ? close_range : range_chance) / 2);
}

int seismic_see(Mech *target, BattleMap *map, int sensor, float range,
                int condition_range, int light) {
  (void)target;
  (void)map;
  (void)sensor;
  (void)condition_range;
  (void)light;
  return (int)(50 - range * 4);
}

int radar_see(Mech *target, BattleMap *map, int sensor, float range,
              int condition_range, int light) {
  (void)target;
  (void)map;
  (void)sensor;
  (void)condition_range;
  (void)light;
  float chance = 180 - range;
  return (int)(chance < 10 ? 10 : chance > 90 ? 90 : chance);
}

int bap_see(Mech *target, BattleMap *map, int sensor, float range,
            int condition_range, int light) {
  (void)target;
  (void)map;
  (void)sensor;
  (void)range;
  (void)condition_range;
  (void)light;
  return 101;
}

int blood_see(Mech *target, BattleMap *map, int sensor, float range,
              int condition_range, int light) {
  return bap_see(target, map, sensor, range, condition_range, light);
}

int vislight_csee(Mech *observer, Mech *target, BattleMap *map, float range,
                  int flags) {
  (void)observer;
  (void)range;
  return !battle_map_sensor_is_disabled(map, SENSOR_VIS) &&
         !(flags & (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_FIRE |
                    BATTLE_MAP_LOS_SMOKE)) &&
         battle_map_los_wood_count(flags) < 3 &&
         (!target || mech_position_z(target) >= 0 ||
          mech_los_actual_elevation(
              btech_context_get_map(mech_context(target),
                                    mech_map_dbref(target)),
              mech_position_x(target), mech_position_y(target),
              target) >= 0.0 ||
          battle_map_los_water_count(flags) < 6);
}

int liteamp_csee(Mech *observer, Mech *target, BattleMap *map, float range,
                 int flags) {
  (void)observer;
  (void)range;
  return !battle_map_sensor_is_disabled(map, SENSOR_LA) &&
         !(flags & (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_FIRE |
                    BATTLE_MAP_LOS_SMOKE)) &&
         (!target || !mech_sensor_is_lit(target)) &&
         battle_map_los_wood_count(flags) < 2 &&
         !battle_map_los_water_count(flags);
}

int infrared_csee(Mech *observer, Mech *target, BattleMap *map, float range,
                  int flags) {
  (void)observer;
  (void)range;
  return !battle_map_sensor_is_disabled(map, SENSOR_IR) &&
         !(flags & (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_FIRE)) &&
         battle_map_los_wood_count(flags) < 6 &&
         (!target || !mech_sensor_target_is_small(target));
}

int electrom_csee(Mech *observer, Mech *target, BattleMap *map, float range,
                  int flags) {
  (void)range;
  return !battle_map_sensor_is_disabled(map, SENSOR_EM) &&
         !(flags & (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_MOUNTAIN)) &&
         battle_map_los_wood_count(flags) < 8 &&
         !mech_is_any_ecm_disturbed(observer) &&
         (!target || mech_class(target) != CLASS_MW);
}

int seismic_csee(Mech *observer, Mech *target, BattleMap *map, float range,
                 int flags) {
  (void)range;
  (void)flags;
  return !battle_map_sensor_is_disabled(map, SENSOR_SE) && target &&
         !mech_is_jumping(observer) &&
         (btech_context_seismic_detects_stopped_units(mech_context(observer)) ||
          fabsf(mech_current_speed(target)) > MP1) &&
         ((mech_movement_type(observer) != MOVE_VTOL ||
           mech_is_landed(observer)) &&
          (mech_movement_type(observer) != MOVE_FLY ||
           mech_is_landed(observer))) &&
         mech_is_started(target) && !mech_is_jumping(target) &&
         mech_class(target) != CLASS_BSUIT && mech_class(target) != CLASS_MW &&
         mech_movement_type(target) != MOVE_HOVER &&
         (mech_movement_type(target) != MOVE_VTOL || mech_is_landed(target)) &&
         (mech_movement_type(target) != MOVE_FLY || mech_is_landed(target)) &&
         mech_movement_type(target) != MOVE_NONE;
}

int radar_csee(Mech *observer, Mech *target, BattleMap *map, float range,
               int flags) {
  (void)observer;
  return !battle_map_sensor_is_disabled(map, SENSOR_RA) && target &&
         mech_position_z(target) > 2 && !(flags & BATTLE_MAP_LOS_BLOCKED) &&
         (mech_position_z(target) >= 10 ||
          range < mech_position_z(target) * mech_position_z(target)) &&
         mech_sensor_elevation_above_surface(target) > 1;
}

int bap_csee(Mech *observer, Mech *target, BattleMap *map, float range,
             int flags) {
  (void)range;
  (void)flags;
  return !battle_map_sensor_is_disabled(map, SENSOR_BAP) &&
         !mech_is_any_ecm_disturbed(observer) && target &&
         !mech_condition_summary(target).angel_ecm_protected &&
         !mech_condition_summary(target).stealth_armor_active &&
         !mech_condition_summary(target).null_signature_active;
}

int blood_csee(Mech *observer, Mech *target, BattleMap *map, float range,
               int flags) {
  (void)range;
  (void)flags;
  return !battle_map_sensor_is_disabled(map, SENSOR_BHAP) &&
         !mech_is_any_ecm_disturbed(observer) && target &&
         !mech_condition_summary(target).angel_ecm_protected;
}

int vislight_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
                   int light) {
  (void)observer;
  (void)map;
  return (!target || !mech_sensor_is_lit(target) ? 2 - light : 0) +
         sensor_woods_count(target, flags) +
         sensor_partial_cover_modifier(target, flags, 3);
}

int liteamp_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
                  int light) {
  (void)observer;
  (void)map;
  return (2 - light) / 2 + sensor_woods_count(target, flags) * 3 / 2 +
         sensor_partial_cover_modifier(target, flags, 3);
}

int infrared_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
                   int light) {
  (void)observer;
  (void)map;
  (void)light;
#ifdef BT_SCALED_INFRARED
  float heat =
      2 * (mech_heat_production(target) - mech_heat_dissipation(target)) +
      fminf(mech_heat_production(target), mech_heat_dissipation(target));
  return sensor_woods_count(target, flags) * 4 / 3 +
         (flags & BATTLE_MAP_LOS_PARTIAL_COVER ? 3 : 0) +
         sensor_heat_modifier(heat);
#else
  return sensor_woods_count(target, flags) * 4 / 3 +
         sensor_partial_cover_modifier(target, flags, 3) +
         sensor_heat_modifier(mech_excess_heat(target) + 7);
#endif
}

int electrom_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
                   int light) {
  (void)map;
  (void)light;
  return sensor_woods_count(target, flags) * 2 / 3 +
         sensor_partial_cover_modifier(target, flags, 3) +
         sensor_weight_modifier(mech_tonnage(target)) +
         sensor_movement_modifier(mech_current_speed(target)) +
         (mech_has_fired_recently(target) ? -1 : 0) + MNumber(observer, 0, 1);
}

int seismic_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
                  int light) {
  (void)map;
  (void)light;
  return 2 + sensor_partial_cover_modifier(target, flags, 3) +
         sensor_weight_modifier(mech_real_tonnage(target)) -
         sensor_movement_modifier(mech_current_speed(target)) +
         MNumber(observer, 0, 1);
}

int bap_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
              int light) {
  (void)target;
  (void)map;
  (void)flags;
  (void)light;
  return MNumber(observer, 0, 2);
}

int blood_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
                int light) {
  return bap_tohit(observer, target, map, flags, light);
}

int radar_tohit(Mech *observer, Mech *target, BattleMap *map, int flags,
                int light) {
  (void)observer;
  (void)map;
  (void)light;
  return (mech_position_z(target) >= 10 || mech_is_flying_type(target) ? -3
                                                                       : 0) +
         sensor_partial_cover_modifier(target, flags, 2) +
         sensor_woods_count(target, flags);
}
