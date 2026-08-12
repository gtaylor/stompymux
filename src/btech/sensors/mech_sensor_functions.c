#include "equipment_types.h"
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
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"
#include "section_types.h"

#include <math.h>

static bool mech_sensor_target_is_small(const Mech *target) {
  return target &&
         (mech_class(target) == CLASS_BSUIT || mech_class(target) == CLASS_MW);
}

static float sensor_range_limit(int range) { return (float)range; }

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

typedef struct SensorPartialCoverRequest {
  const Mech *target;
  int flags;
  int base;
} SensorPartialCoverRequest;

static int
sensor_partial_cover_modifier(const SensorPartialCoverRequest *request) {
  if (!(request->flags & BATTLE_MAP_LOS_PARTIAL_COVER))
    return 0;
  return request->base +
         (mech_condition_summary(request->target).hull_down ? 2 : 0);
}

static int sensor_heat_modifier(float heat) {
#ifdef BT_SCALED_INFRARED
  if (heat <= 0)
    return 2;
  if (heat > 50)
    return -2;
  if (heat > 35)
    return -1;
  return heat > 20 ? 0 : 1;
#else
  if (heat <= 7)
    return 2;
  if (heat <= 10)
    return 1;
  if (heat <= 15)
    return 0;
  return heat <= 22 ? -1 : -2;
#endif
}

static int sensor_weight_modifier(int tonnage) {
  if (tonnage > 65)
    return -1;
  return tonnage > 35 ? 0 : 1;
}

static int sensor_movement_modifier(float speed) {
  return fabsf(speed) >= 10.75F ? 1 : 0;
}

int vislight_see(const SensorVisibilityRequest *request) {
  int illuminated_multiplier =
      !request->light && request->target && mech_sensor_is_lit(request->target)
          ? 3
          : 1;
  const int MAXIMUM_RANGE = request->condition_range * illuminated_multiplier;
  if (request->range > sensor_range_limit(MAXIMUM_RANGE))
    return 0;
  return (int)((100 - (request->range / 3)) /
               (mech_sensor_target_is_small(request->target) ? 3 : 1));
}

int liteamp_see(const SensorVisibilityRequest *request) {
  const int MAXIMUM_RANGE =
      request->light ? request->condition_range : 2 * request->condition_range;
  if (request->range > sensor_range_limit(MAXIMUM_RANGE))
    return 0;
  return (int)((70 - request->range) /
               (mech_sensor_target_is_small(request->target) ? 3 : 1));
}

int infrared_see(const SensorVisibilityRequest *request) {
  return (int)(80 - request->range);
}

int electrom_see(const SensorVisibilityRequest *request) {
  float close_range = request->range < 24 ? 2 : 0;
  float range_chance = 60 - (request->range * 2);
  return (int)((close_range > range_chance ? close_range : range_chance) / 2);
}

int seismic_see(const SensorVisibilityRequest *request) {
  return (int)(50 - (request->range * 4));
}

int radar_see(const SensorVisibilityRequest *request) {
  float chance = 180 - request->range;
  if (chance < 10)
    return 10;
  return (int)(chance > 90 ? 90 : chance);
}

int bap_see(const SensorVisibilityRequest *request) {
  (void)request;
  return 101;
}

int blood_see(const SensorVisibilityRequest *request) {
  return bap_see(request);
}

int vislight_csee(const SensorContactRequest *request) {
  return !battle_map_sensor_is_disabled(request->map, SENSOR_VIS) &&
         !(request->flags & (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_FIRE |
                             BATTLE_MAP_LOS_SMOKE)) &&
         battle_map_los_wood_count(request->flags) < 3 &&
         (!request->target || mech_position_z(request->target) >= 0 ||
          mech_los_actual_elevation(
              btech_context_get_map(mech_context(request->target),
                                    mech_map_dbref(request->target)),
              mech_position_x(request->target),
              mech_position_y(request->target), request->target) >= 0.0F ||
          battle_map_los_water_count(request->flags) < 6);
}

int liteamp_csee(const SensorContactRequest *request) {
  return !battle_map_sensor_is_disabled(request->map, SENSOR_LA) &&
         !(request->flags & (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_FIRE |
                             BATTLE_MAP_LOS_SMOKE)) &&
         (!request->target || !mech_sensor_is_lit(request->target)) &&
         battle_map_los_wood_count(request->flags) < 2 &&
         !battle_map_los_water_count(request->flags);
}

int infrared_csee(const SensorContactRequest *request) {
  return !battle_map_sensor_is_disabled(request->map, SENSOR_IR) &&
         !(request->flags & (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_FIRE)) &&
         battle_map_los_wood_count(request->flags) < 6 &&
         (!request->target || !mech_sensor_target_is_small(request->target));
}

int electrom_csee(const SensorContactRequest *request) {
  return !battle_map_sensor_is_disabled(request->map, SENSOR_EM) &&
         !(request->flags &
           (BATTLE_MAP_LOS_BLOCKED | BATTLE_MAP_LOS_MOUNTAIN)) &&
         battle_map_los_wood_count(request->flags) < 8 &&
         !mech_is_any_ecm_disturbed(request->observer) &&
         (!request->target || mech_class(request->target) != CLASS_MW);
}

int seismic_csee(const SensorContactRequest *request) {
  return !battle_map_sensor_is_disabled(request->map, SENSOR_SE) &&
         request->target && !mech_is_jumping(request->observer) &&
         (btech_context_seismic_detects_stopped_units(
              mech_context(request->observer)) ||
          fabsf(mech_current_speed(request->target)) > MP1) &&
         ((mech_movement_type(request->observer) != MOVE_VTOL ||
           mech_is_landed(request->observer)) &&
          (mech_movement_type(request->observer) != MOVE_FLY ||
           mech_is_landed(request->observer))) &&
         mech_is_started(request->target) &&
         !mech_is_jumping(request->target) &&
         mech_class(request->target) != CLASS_BSUIT &&
         mech_class(request->target) != CLASS_MW &&
         mech_movement_type(request->target) != MOVE_HOVER &&
         (mech_movement_type(request->target) != MOVE_VTOL ||
          mech_is_landed(request->target)) &&
         (mech_movement_type(request->target) != MOVE_FLY ||
          mech_is_landed(request->target)) &&
         mech_movement_type(request->target) != MOVE_NONE;
}

int radar_csee(const SensorContactRequest *request) {
  if (battle_map_sensor_is_disabled(request->map, SENSOR_RA) ||
      !request->target || mech_position_z(request->target) <= 2 ||
      (request->flags & BATTLE_MAP_LOS_BLOCKED) ||
      mech_sensor_elevation_above_surface(request->target) <= 1)
    return 0;
  const int TARGET_Z = mech_position_z(request->target);
  const float TARGET_Z_SQUARED = (float)TARGET_Z * (float)TARGET_Z;
  return TARGET_Z >= 10 || request->range < TARGET_Z_SQUARED;
}

int bap_csee(const SensorContactRequest *request) {
  return !battle_map_sensor_is_disabled(request->map, SENSOR_BAP) &&
         !mech_is_any_ecm_disturbed(request->observer) && request->target &&
         !mech_condition_summary(request->target).angel_ecm_protected &&
         !mech_condition_summary(request->target).stealth_armor_active &&
         !mech_condition_summary(request->target).null_signature_active;
}

int blood_csee(const SensorContactRequest *request) {
  return !battle_map_sensor_is_disabled(request->map, SENSOR_BHAP) &&
         !mech_is_any_ecm_disturbed(request->observer) && request->target &&
         !mech_condition_summary(request->target).angel_ecm_protected;
}

int vislight_tohit(const SensorToHitRequest *request) {
  return (!request->target || !mech_sensor_is_lit(request->target)
              ? 2 - request->light
              : 0) +
         sensor_woods_count(request->target, request->flags) +
         sensor_partial_cover_modifier(&(SensorPartialCoverRequest){
             .target = request->target, .flags = request->flags, .base = 3});
}

int liteamp_tohit(const SensorToHitRequest *request) {
  return ((2 - request->light) / 2) +
         (sensor_woods_count(request->target, request->flags) * 3 / 2) +
         sensor_partial_cover_modifier(&(SensorPartialCoverRequest){
             .target = request->target, .flags = request->flags, .base = 3});
}

int infrared_tohit(const SensorToHitRequest *request) {
#ifdef BT_SCALED_INFRARED
  float heat = (2 * (mech_heat_production(request->target) -
                     mech_heat_dissipation(request->target))) +
               fminf(mech_heat_production(request->target),
                     mech_heat_dissipation(request->target));
  return (sensor_woods_count(request->target, request->flags) * 4 / 3) +
         (request->flags & BATTLE_MAP_LOS_PARTIAL_COVER ? 3 : 0) +
         sensor_heat_modifier(heat);
#else
  return sensor_woods_count(request->target, request->flags) * 4 / 3 +
         sensor_partial_cover_modifier(&(SensorPartialCoverRequest){
             .target = request->target, .flags = request->flags, .base = 3}) +
         sensor_heat_modifier(mech_excess_heat(request->target) + 7);
#endif
}

int electrom_tohit(const SensorToHitRequest *request) {
  return (sensor_woods_count(request->target, request->flags) * 2 / 3) +
         sensor_partial_cover_modifier(&(SensorPartialCoverRequest){
             .target = request->target, .flags = request->flags, .base = 3}) +
         sensor_weight_modifier(mech_tonnage(request->target)) +
         sensor_movement_modifier(mech_current_speed(request->target)) +
         (mech_has_fired_recently(request->target) ? -1 : 0) +
         m_number(request->observer, 0, 1);
}

int seismic_tohit(const SensorToHitRequest *request) {
  return 2 +
         sensor_partial_cover_modifier(&(SensorPartialCoverRequest){
             .target = request->target, .flags = request->flags, .base = 3}) +
         sensor_weight_modifier(mech_real_tonnage(request->target)) -
         sensor_movement_modifier(mech_current_speed(request->target)) +
         m_number(request->observer, 0, 1);
}

int bap_tohit(const SensorToHitRequest *request) {
  return m_number(request->observer, 0, 2);
}

int blood_tohit(const SensorToHitRequest *request) {
  return bap_tohit(request);
}

int radar_tohit(const SensorToHitRequest *request) {
  return (mech_position_z(request->target) >= 10 ||
                  mech_is_flying_type(request->target)
              ? -3
              : 0) +
         sensor_partial_cover_modifier(&(SensorPartialCoverRequest){
             .target = request->target, .flags = request->flags, .base = 2}) +
         sensor_woods_count(request->target, request->flags);
}
