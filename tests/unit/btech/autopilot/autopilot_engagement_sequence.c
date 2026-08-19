/*
 * Walks the pure policy decisions an engaged autopilot makes across one
 * combat turn: route, sensors, approach, weapons, and physical attacks. Each
 * policy has its own focused suite; this one pins the hand-offs between them,
 * where a decision made by one policy becomes the input of the next.
 */

#include <math.h>

#include "autopilot_combat_policy_api.h"
#include "autopilot_movement_policy_api.h"
#include "autopilot_path_policy_api.h"
#include "autopilot_sensor_policy_api.h"
#include "map_terrain.h"
#include "mech_sensor.h"
#include "mux/support/checked_storage.h"

static AutopilotPathHex *hex_at(AutopilotPathHex *hexes, size_t count,
                                int height, int x, int y) {
  return checked_storage_at(hexes, count, sizeof(*hexes),
                            (size_t)((x * height) + y));
}

static const AutopilotPathPoint *point_at(const AutopilotPathResult *result,
                                          size_t index) {
  return checked_storage_at_const(result->points, result->count,
                                  sizeof(*result->points), index);
}

static bool nearly_equal(float left, float right) {
  return fabsf(left - right) < 0.0001F;
}

/* A wheeled unit must route around water to reach a firing position. */
static int test_route_to_contact(void) {
  enum { WIDTH = 5, HEIGHT = 3 };
  AutopilotPathHex hexes[WIDTH * HEIGHT] = {0};
  for (int x = 0; x < WIDTH; x++)
    for (int y = 0; y < HEIGHT; y++)
      hex_at(hexes, WIDTH * HEIGHT, HEIGHT, x, y)->terrain =
          BATTLE_TERRAIN_GRASSLAND;
  hex_at(hexes, WIDTH * HEIGHT, HEIGHT, 2, 1)->terrain = BATTLE_TERRAIN_WATER;

  AutopilotPathResult path = autopilot_path_find(
      &(AutopilotPathRequest){.width = WIDTH,
                              .height = HEIGHT,
                              .hexes = hexes,
                              .start = {.x = 0, .y = 1},
                              .goal = {.x = 4, .y = 1},
                              .mobility = AUTOPILOT_PATH_WHEELED});
  const bool PATH_FOUND = path.status == AUTOPILOT_PATH_FOUND && path.count > 0;
  bool path_uses_water = false;
  for (size_t index = 0; index < path.count; index++)
    if (point_at(&path, index)->x == 2 && point_at(&path, index)->y == 1)
      path_uses_water = true;
  autopilot_path_result_destroy(&path);
  if (!PATH_FOUND)
    return 1;
  return path_uses_water ? 2 : 0;
}

static int test_sensors_for_contact(void) {
  /* A nearby heavy opponent takes the engagement sensors regardless of how
   * good the light is. */
  const AutopilotSensorSituation HEAVY = {.has_target = true,
                                          .target_range = 12,
                                          .target_tonnage = 75,
                                          .preferred_visual_sensor = SENSOR_VIS,
                                          .effective_visibility = 30};
  AutopilotSensorSelection selection = autopilot_sensor_select(&HEAVY);
  if (selection.primary != SENSOR_EM || selection.secondary != SENSOR_IR)
    return 1;

  /* A light opponent keeps the visual sensor the light conditions chose, and
   * darkness is what pulls the secondary onto EM. */
  AutopilotSensorSituation light = HEAVY;
  light.target_tonnage = 30;
  light.preferred_visual_sensor =
      autopilot_visual_sensor_select(false, false, 1, 0);
  if (light.preferred_visual_sensor != SENSOR_LA)
    return 2;
  selection = autopilot_sensor_select(&light);
  if (selection.primary != SENSOR_LA || selection.secondary != SENSOR_LA)
    return 3;
  light.effective_visibility = 6;
  selection = autopilot_sensor_select(&light);
  if (selection.primary != SENSOR_LA || selection.secondary != SENSOR_EM)
    return 4;
  return 0;
}

/* The route enters the approach zone: turn before advancing, then close. */
static int test_close_on_target(void) {
  if (autopilot_approach_evaluate(
          &(AutopilotApproachSituation){
              .range = 1.0F, .bearing = 10, .heading = 320, .at_target = false})
          .action != AUTOPILOT_APPROACH_TURN_AND_STOP)
    return 1;
  const AutopilotApproachDecision APPROACH =
      autopilot_approach_evaluate(&(AutopilotApproachSituation){
          .range = 1.0F, .bearing = 10, .heading = 350, .at_target = false});
  if (APPROACH.action != AUTOPILOT_APPROACH_SLOW ||
      !nearly_equal(APPROACH.speed_ratio, 0.9F))
    return 2;
  if (!autopilot_cruise_should_accelerate(&(AutopilotCruiseSituation){
          .bearing = 10, .heading = 350, .desired_speed = 0.0F}))
    return 3;
  return nearly_equal(autopilot_cruise_speed_ratio(false), 1.0F) ? 0 : 4;
}

/* Two shots of the same weapon fit the heat budget once, not twice, and the
 * carried-forward projection is what closes the second shot out. */
static int test_weapons_and_heat(void) {
  const AutopilotWeaponSituation READY = {.functional = true,
                                          .ammunition_compatible = true,
                                          .in_arc = true,
                                          .heat_limited = true,
                                          .projected_heat = 16.0F,
                                          .heat_dissipation = 4.0F,
                                          .weapon_heat = 8,
                                          .maximum_heat = 20.0F};
  const AutopilotWeaponDecision FIRST = autopilot_weapon_evaluate(&READY);
  if (!FIRST.fire || !nearly_equal(FIRST.heat_after_fire, 24.0F))
    return 1;

  AutopilotWeaponSituation second = READY;
  second.projected_heat = FIRST.heat_after_fire;
  const AutopilotWeaponDecision SECOND = autopilot_weapon_evaluate(&second);
  if (SECOND.fire || SECOND.reason != AUTOPILOT_WEAPON_OVERHEAT ||
      !nearly_equal(SECOND.heat_after_fire, 24.0F))
    return 2;
  return 0;
}

static int test_physical_fallback(void) {
  if (autopilot_physical_choose_leg(true, 3, true, 1) !=
      AUTOPILOT_PHYSICAL_LEFT)
    return 1;
  if (autopilot_physical_choose_punch(false, true) != AUTOPILOT_PHYSICAL_LEFT ||
      autopilot_physical_choose_punch(false, false) != AUTOPILOT_PHYSICAL_NONE)
    return 2;
  return 0;
}

int main(void) {
  const int ROUTE = test_route_to_contact();
  if (ROUTE)
    return 10 + ROUTE;
  const int SENSOR_CHOICE = test_sensors_for_contact();
  if (SENSOR_CHOICE)
    return 20 + SENSOR_CHOICE;
  const int CLOSING = test_close_on_target();
  if (CLOSING)
    return 30 + CLOSING;
  const int WEAPONS = test_weapons_and_heat();
  if (WEAPONS)
    return 40 + WEAPONS;
  const int PHYSICAL = test_physical_fallback();
  return PHYSICAL ? 50 + PHYSICAL : 0;
}
