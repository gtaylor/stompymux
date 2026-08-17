/*
 * los_map_calculate scenarios, covering map_los.c:
 * - clairvoyance, terrainless sensors, and input guards (675,707-714)
 * - range, ridge, woods, mountain, and water terrain tracing (255,553,588,
 *   599-619,643,655)
 * - underwater and both-world water setup (691-702)
 * - map fire, jelly, and searchlight lighting (415-420,441,454,461)
 * - searchlight range/arc limits and weather-dependent sensor range,
 *   including lit out-of-range hexes (278,280,307,589,631)
 * - per-sensor terrain obstruction callbacks and even-column neighbor parity
 *   (222-249,441-459)
 * - map-hex boundary reads return the error cell without aliasing another row
 *   (133)
 * - searchlight unit-class height behavior (147-167)
 */

#include "btech_los_fixture.h"
#include "btech_los_test.h"

#include "map_los.h"
#include "map_terrain.h"

static bool calculate(HexLosMap *los_map, BattleMap *map, Mech *observer,
                      int start_x, int start_y, int width, int height) {
  return los_map_calculate(los_map, map, observer, start_x, start_y, width,
                           height);
}

static bool hex_has(const HexLosMap *los_map, int x, int y, int flag) {
  return (los_map_flag(los_map, x, y) & flag) != 0;
}

static void test_map_hex_bounds(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  HexLosMap los_map = {
      .startx = 10,
      .starty = 20,
      .xsize = 2,
      .ysize = 2,
      .map = {MAPLOSHEX_LIT, MAPLOSHEX_SEE, MAPLOSHEX_SEEELEV,
              MAPLOSHEX_SEETERRAIN},
  };
  /* Non-zero-origin map indexing at map_los.c:133-145. */
  los_expect_int(state, "origin hex reads the first map cell", MAPLOSHEX_LIT,
                 los_map_flag(&los_map, 10, 20));
  los_expect_int(state, "top-right hex reads the second map cell",
                 MAPLOSHEX_SEE, los_map_flag(&los_map, 11, 20));
  los_expect_int(state, "bottom-left hex reads the third map cell",
                 MAPLOSHEX_SEEELEV, los_map_flag(&los_map, 10, 21));
  los_expect_int(state, "bottom-right hex reads the fourth map cell",
                 MAPLOSHEX_SEETERRAIN, los_map_flag(&los_map, 11, 21));

  los_fixture_reset(&map);
  los_map.map[0] = 0;
  /* Left map boundary at map_los.c:133-141. */
  los_expect_int(state, "left out-of-bounds hex returns the error cell", 0,
                 los_map_flag(&los_map, 9, 20));
  los_expect_int(state, "left out-of-bounds hex reports an LOS error", 1,
                 los_fixture_channel_error_count());
  /* Right map boundary at map_los.c:133-141. */
  los_expect_int(state, "right out-of-bounds hex returns the error cell", 0,
                 los_map_flag(&los_map, 12, 20));
  los_expect_int(state, "right out-of-bounds hex reports an LOS error", 2,
                 los_fixture_channel_error_count());
  /* Top map boundary at map_los.c:133-141. */
  los_expect_int(state, "top out-of-bounds hex returns the error cell", 0,
                 los_map_flag(&los_map, 10, 19));
  los_expect_int(state, "top out-of-bounds hex reports an LOS error", 3,
                 los_fixture_channel_error_count());
  /* Bottom map boundary at map_los.c:133-141. */
  los_expect_int(state, "bottom out-of-bounds hex returns the error cell", 0,
                 los_map_flag(&los_map, 10, 22));
  los_expect_int(state, "bottom out-of-bounds hex reports an LOS error", 4,
                 los_fixture_channel_error_count());
}

static bool surface_water_traces(MechMovementType movement, char terrain) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  observer.movement = movement;
  observer.terrain = terrain;
  los_fixture_set_hex(2, 3, WATER, WATER, 0);
  calculate(&los_map, &map, &observer, 2, 2, 1, 3);
  return hex_has(&los_map, 2, 3, MAPLOSHEX_SEE);
}

static void test_clairvoyance(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  observer.clairvoyant = true;
  const bool calculated = calculate(&los_map, &map, &observer, 0, 0, 4, 3);

  /* Clairvoyant map fill at map_los.c:707-710. */
  los_expect_true(state, "clairvoyance calculates the map", calculated);
  los_expect_true(state,
                  "clairvoyance grants the first hex terrain and elevation",
                  hex_has(&los_map, 0, 0, MAPLOSHEX_SEE));
  los_expect_true(state,
                  "clairvoyance grants the last hex terrain and elevation",
                  hex_has(&los_map, 3, 2, MAPLOSHEX_SEE));
}

static void test_terrainless_sensors(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  observer.sensors[0] = 4;
  observer.sensors[1] = 5;
  const bool calculated = calculate(&los_map, &map, &observer, 0, 0, 4, 3);

  /* Terrainless sensor shortcut at map_los.c:712-715. */
  los_expect_true(state, "non-terrain sensors calculate the map", calculated);
  los_expect_true(state, "non-terrain sensors mark the first hex NOLOS",
                  !hex_has(&los_map, 0, 0, MAPLOSHEX_SEE));
  los_expect_true(state, "non-terrain sensors mark the last hex NOLOS",
                  !hex_has(&los_map, 3, 2, MAPLOSHEX_SEE));
}

static void test_size_guards(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);

  /* Lower x-size guard at map_los.c:675-681. */
  los_expect_true(state, "zero x-size is rejected",
                  !calculate(&los_map, &map, &observer, 0, 0, 0, 1));
  /* Lower y-size guard at map_los.c:675-681. */
  los_expect_true(state, "zero y-size is rejected",
                  !calculate(&los_map, &map, &observer, 0, 0, 1, 0));
  /* Upper x-size guard at map_los.c:675-681. */
  los_expect_true(
      state, "x-size above MAPLOS_MAXX is rejected",
      !calculate(&los_map, &map, &observer, 0, 0, MAPLOS_MAXX + 1, 1));
  /* Upper y-size guard at map_los.c:675-681. */
  los_expect_true(
      state, "y-size above MAPLOS_MAXY is rejected",
      !calculate(&los_map, &map, &observer, 0, 0, 1, MAPLOS_MAXY + 1));
}

static void test_flat_map_range(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  map.mapvis = 3;
  Mech observer = los_fixture_make_mech(0, 0, 0);
  const bool calculated = calculate(&los_map, &map, &observer, 0, 0, 6, 1);

  /* Sensor range boundary in mech_los_sees_range at map_los.c:255-300. */
  los_expect_true(state, "flat map range calculation succeeds", calculated);
  los_expect_true(state, "flat map sees inside sensor range",
                  hex_has(&los_map, 2, 0, MAPLOSHEX_SEE));
  los_expect_true(state, "flat map sees within the hex-metric sensor range",
                  hex_has(&los_map, 3, 0, MAPLOSHEX_SEE));
}

static void test_ridge_occlusion(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  los_fixture_set_hex(2, 3, GRASSLAND, GRASSLAND, 2);
  const bool calculated = calculate(&los_map, &map, &observer, 2, 2, 1, 4);

  /* Minimum-angle ridge occlusion at map_los.c:553 and 588-595. */
  los_expect_true(state, "ridge map calculation succeeds", calculated);
  los_expect_true(state, "level-two ridge hides lower hex behind it",
                  !hex_has(&los_map, 2, 4, MAPLOSHEX_SEE));
}

static void test_woods_obstruction(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  los_fixture_sensor_set(0, 60, true, 0, 99, true);
  los_fixture_sensor_set(1, 60, true, 0, 99, true);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  los_fixture_set_hex(2, 3, LIGHT_FOREST, LIGHT_FOREST, 0);
  los_fixture_set_hex(2, 4, LIGHT_FOREST, LIGHT_FOREST, 0);
  los_fixture_set_hex(2, 5, LIGHT_FOREST, LIGHT_FOREST, 0);
  const bool light_calculated =
      calculate(&los_map, &map, &observer, 2, 2, 1, 5);
  const bool light_blocked = !hex_has(&los_map, 2, 6, MAPLOSHEX_SEE);

  los_fixture_reset(&map);
  los_fixture_sensor_set(0, 60, true, 2, 99, true);
  los_fixture_sensor_set(1, 60, true, 2, 99, true);
  observer = los_fixture_make_mech(2, 2, 0);
  los_fixture_set_hex(2, 3, HEAVY_FOREST, HEAVY_FOREST, 0);
  los_fixture_set_hex(2, 4, LIGHT_FOREST, LIGHT_FOREST, 0);
  const bool heavy_calculated =
      calculate(&los_map, &map, &observer, 2, 2, 1, 4);

  /* Light-forest obstruction counting at map_los.c:599-619. */
  los_expect_true(state, "light forest map calculation succeeds",
                  light_calculated);
  los_expect_true(state, "light forests accumulate and block the next hex",
                  light_blocked);
  /* Heavy-forest obstruction counting at map_los.c:599-619. */
  los_expect_true(state, "heavy forest map calculation succeeds",
                  heavy_calculated);
  los_expect_true(state, "heavy forest contributes two obstruction points",
                  !hex_has(&los_map, 2, 5, MAPLOSHEX_SEE));
}

static void test_mountain_and_water_blocks(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  los_fixture_sensor_set(0, 60, true, 99, 99, false);
  los_fixture_sensor_set(1, 60, true, 99, 99, false);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  los_fixture_set_hex(2, 3, MOUNTAINS, MOUNTAINS, 0);
  const bool mountain_calculated =
      calculate(&los_map, &map, &observer, 2, 2, 1, 4);

  los_fixture_reset(&map);
  los_fixture_sensor_set(0, 60, true, 99, 0, true);
  los_fixture_sensor_set(1, 60, true, 99, 0, true);
  observer = los_fixture_make_mech(2, 2, 0);
  los_fixture_set_hex(2, 3, WATER, WATER, 0);
  const bool water_calculated =
      calculate(&los_map, &map, &observer, 2, 2, 1, 4);

  /* Mountain sensor obstruction at map_los.c:655-660. */
  los_expect_true(state, "mountain map calculation succeeds",
                  mountain_calculated);
  los_expect_true(state, "mountain blocks a sensor that cannot see over it",
                  !hex_has(&los_map, 2, 4, MAPLOSHEX_SEE));
  /* Water obstruction minimum-angle pin at map_los.c:643-652. */
  los_expect_true(state, "water map calculation succeeds", water_calculated);
  los_expect_true(state, "water blocks and hides hexes beyond it",
                  !hex_has(&los_map, 2, 4, MAPLOSHEX_SEE));
}

static void test_sensor_specific_obstruction_limits(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  los_fixture_sensor_set(0, 60, true, 99, 99, false);
  los_fixture_sensor_set(1, 60, true, 99, 99, true);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  los_fixture_set_hex(2, 3, MOUNTAINS, MOUNTAINS, 0);
  const bool calculated = calculate(&los_map, &map, &observer, 2, 2, 1, 4);

  /* Sensor-specific mountain callback dispatch at map_los.c:228-249,655-660. */
  los_expect_true(state, "per-sensor obstruction map calculation succeeds",
                  calculated);
  los_expect_true(state, "second sensor uses its own mountain limit",
                  hex_has(&los_map, 2, 4, MAPLOSHEX_SEE));
}

static void test_water_world_setup(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  Mech observer;

  los_fixture_reset(&map);
  observer = los_fixture_make_mech(2, 2, -1);
  observer.terrain = WATER;
  los_fixture_set_hex(2, 3, WATER, WATER, 0);
  calculate(&los_map, &map, &observer, 2, 2, 1, 3);
  /* Submerged water tracing setup at map_los.c:691-702. */
  los_expect_true(state, "submerged observer traces water hexes",
                  hex_has(&los_map, 2, 3, MAPLOSHEX_SEE));

  /* Hull both-world setup at map_los.c:691-702. */
  los_expect_true(state, "hull on ice traces water",
                  surface_water_traces(MOVE_HULL, ICE));
  los_expect_true(state, "hull on water traces water",
                  surface_water_traces(MOVE_HULL, WATER));
  los_expect_true(state, "hull on bridge traces water",
                  surface_water_traces(MOVE_HULL, BRIDGE));
  /* Foil both-world setup at map_los.c:691-702. */
  los_expect_true(state, "foil on ice traces water",
                  surface_water_traces(MOVE_FOIL, ICE));
  los_expect_true(state, "foil on water traces water",
                  surface_water_traces(MOVE_FOIL, WATER));
  los_expect_true(state, "foil on bridge traces water",
                  surface_water_traces(MOVE_FOIL, BRIDGE));
  /* Hover both-world setup at map_los.c:691-702. */
  los_expect_true(state, "hover on ice traces water",
                  surface_water_traces(MOVE_HOVER, ICE));
  los_expect_true(state, "hover on water traces water",
                  surface_water_traces(MOVE_HOVER, WATER));
  los_expect_true(state, "hover on bridge traces water",
                  surface_water_traces(MOVE_HOVER, BRIDGE));
}

static void test_litemark_map(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  MapObject fire = {.x = 4, .y = 4};
  los_fixture_reset(&map);
  map.map_object[TYPE_FIRE] = &fire;
  Mech observer = los_fixture_make_mech(2, 2, 0);
  calculate(&los_map, &map, &observer, 2, 2, 5, 5);

  /* Fire map-object lighting at map_los.c:441-444. */
  los_expect_true(state, "fire map object lights its hex",
                  hex_has(&los_map, 4, 4, MAPLOSHEX_LIT));
  los_expect_true(state, "fire map object lights neighboring hexes",
                  hex_has(&los_map, 4, 3, MAPLOSHEX_LIT));
  los_expect_true(state, "fire uses even-column neighbor parity",
                  hex_has(&los_map, 5, 5, MAPLOSHEX_LIT));

  los_fixture_reset(&map);
  observer = los_fixture_make_mech(2, 2, 0);
  Mech jellied = los_fixture_make_mech(4, 4, 0);
  jellied.dbref = 10;
  jellied.jellied = true;
  los_fixture_map_unit_set(0, &jellied);
  calculate(&los_map, &map, &observer, 2, 2, 5, 5);

  /* Jellied-unit lighting at map_los.c:454-459. */
  los_expect_true(state, "jellied unit lights its hex",
                  hex_has(&los_map, 4, 4, MAPLOSHEX_LIT));
  los_expect_true(state, "jellied unit lights neighboring hexes",
                  hex_has(&los_map, 4, 3, MAPLOSHEX_LIT));

  los_fixture_reset(&map);
  observer = los_fixture_make_mech(2, 2, 0);
  Mech searchlight = los_fixture_make_mech(2, 2, 0);
  searchlight.dbref = 11;
  searchlight.searchlight = true;
  los_fixture_map_unit_set(0, &searchlight);
  calculate(&los_map, &map, &observer, 2, 2, 5, 5);

  /* Searchlight trace setup at map_los.c:461-470. */
  los_expect_true(state, "searchlight sets the LOS searchlight flag",
                  los_map.flags & MAPLOS_FLAG_SLITE);
}

static bool searchlight_is_occluded_by(char terrain) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech searchlight = los_fixture_make_mech(2, 2, 0);
  searchlight.dbref = 12;
  searchlight.searchlight = true;
  los_fixture_map_unit_set(0, &searchlight);
  los_fixture_set_hex(2, 3, terrain, terrain, 0);
  calculate(&los_map, &map, &observer, 2, 2, 1, 3);
  return !hex_has(&los_map, 2, 4, MAPLOSHEX_LIT);
}

static void test_searchlight_tracing(LosTestState *state) {
  /* Heavy forest beam angle at map_los.c:415-420. */
  los_expect_true(state, "heavy forest occludes the searchlight beam",
                  searchlight_is_occluded_by(HEAVY_FOREST));
  /* Light forest beam angle at map_los.c:415-420. */
  los_expect_true(state, "light forest occludes the searchlight beam",
                  searchlight_is_occluded_by(LIGHT_FOREST));
  /* Smoke beam angle at map_los.c:415-420. */
  los_expect_true(state, "smoke occludes the searchlight beam",
                  searchlight_is_occluded_by(SMOKE));

  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(0, 0, 0);
  Mech searchlight = los_fixture_make_mech(0, 0, 0);
  searchlight.dbref = 13;
  searchlight.searchlight = true;
  los_fixture_map_unit_set(0, &searchlight);
  calculate(&los_map, &map, &observer, 0, 0, MAPLOS_MAXX, 31);

  /* Searchlight 60-range cutoff at map_los.c:307-327. */
  los_expect_true(state, "searchlight does not light beyond range 60",
                  !hex_has(&los_map, 61, 30, MAPLOSHEX_LIT));

  los_fixture_reset(&map);
  observer = los_fixture_make_mech(0, 0, 0);
  searchlight = los_fixture_make_mech(0, 0, 0);
  searchlight.dbref = 14;
  searchlight.searchlight = true;
  searchlight.weapon_arc = 0;
  los_fixture_map_unit_set(0, &searchlight);
  calculate(&los_map, &map, &observer, 0, 0, 3, 1);

  /* Searchlight forward/turret arc at map_los.c:307-320. */
  los_expect_true(state, "searchlight does not light outside its arc",
                  !hex_has(&los_map, 2, 0, MAPLOSHEX_LIT));
}

static bool searchlight_height_reaches_second_hex(UnitClass unit_class,
                                                  bool fallen) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech searchlight = los_fixture_make_mech(2, 2, 0);
  searchlight.dbref = 15;
  searchlight.searchlight = true;
  searchlight.unit_class = unit_class;
  searchlight.fallen = fallen;
  los_fixture_map_unit_set(0, &searchlight);
  los_fixture_set_hex(2, 3, GRASSLAND, GRASSLAND, 1);
  los_fixture_set_hex(2, 4, GRASSLAND, GRASSLAND, 1);
  calculate(&los_map, &map, &observer, 2, 2, 1, 3);
  return hex_has(&los_map, 2, 4, MAPLOSHEX_LIT);
}

static void test_searchlight_unit_heights(LosTestState *state) {
  /* Standing mech height at map_los.c:149-150. */
  los_expect_true(state, "standing mech searchlight clears the ridge",
                  searchlight_height_reaches_second_hex(CLASS_MECH, false));
  /* Fallen mech height at map_los.c:149-150. */
  los_expect_true(state, "fallen mech searchlight stays below the ridge",
                  !searchlight_height_reaches_second_hex(CLASS_MECH, true));
  /* Spheroid DropShip height at map_los.c:151-152. */
  los_expect_true(
      state, "spheroid dropship searchlight clears the ridge",
      searchlight_height_reaches_second_hex(CLASS_SPHEROID_DS, false));
  /* Aerodyne DropShip height at map_los.c:153-154. */
  los_expect_true(state, "aerodyne dropship searchlight clears the ridge",
                  searchlight_height_reaches_second_hex(CLASS_DS, false));
  /* MechWarrior height at map_los.c:155-157. */
  los_expect_true(state, "mechwarrior searchlight stays below the ridge",
                  !searchlight_height_reaches_second_hex(CLASS_MW, false));
  /* Naval vehicle height at map_los.c:155-157. */
  los_expect_true(
      state, "naval searchlight stays below the ridge",
      !searchlight_height_reaches_second_hex(CLASS_VEH_NAVAL, false));
  /* Ground vehicle height at map_los.c:158-166. */
  los_expect_true(
      state, "ground vehicle searchlight stays below the ridge",
      !searchlight_height_reaches_second_hex(CLASS_VEH_GROUND, false));
  /* VTOL height at map_los.c:158-166. */
  los_expect_true(state, "VTOL searchlight stays below the ridge",
                  !searchlight_height_reaches_second_hex(CLASS_VTOL, false));
  /* Aerospace height at map_los.c:158-166. */
  los_expect_true(state, "aerospace searchlight stays below the ridge",
                  !searchlight_height_reaches_second_hex(CLASS_AERO, false));
  /* Battle armor height at map_los.c:158-166. */
  los_expect_true(state, "battle armor searchlight stays below the ridge",
                  !searchlight_height_reaches_second_hex(CLASS_BSUIT, false));
}

static bool sensor_sees_hex(int sensor, int map_visibility, int map_light,
                            int target_x) {
  BattleMap map;
  HexLosMap los_map;
  los_fixture_reset(&map);
  map.mapvis = (char)map_visibility;
  map.maplight = (char)map_light;
  Mech observer = los_fixture_make_mech(0, 0, 0);
  observer.sensors[0] = sensor;
  observer.sensors[1] = 4;
  calculate(&los_map, &map, &observer, 0, 0, target_x + 1, 1);
  return hex_has(&los_map, target_x, 0, MAPLOSHEX_SEE);
}

static void test_weather_sensor_ranges(LosTestState *state) {
  /* Visual map-visibility clamp at map_los.c:278-279. */
  los_expect_true(state, "map visibility clamps visual sensor range",
                  !sensor_sees_hex(0, 3, 1, 4));
  /* Light-amplification map-visibility clamp at map_los.c:278-279. */
  los_expect_true(state, "map visibility clamps light sensor range",
                  !sensor_sees_hex(1, 3, 1, 4));
  /* Darkness light-amplification extension at map_los.c:280-281. */
  los_expect_true(state, "darkness doubles light sensor range",
                  sensor_sees_hex(1, 3, 0, 5));
}

static void test_lit_out_of_range_hexes(LosTestState *state) {
  BattleMap map;
  HexLosMap los_map;
  MapObject fire = {.x = 1, .y = 1};
  los_fixture_reset(&map);
  map.mapvis = 3;
  map.map_object[TYPE_FIRE] = &fire;
  Mech observer = los_fixture_make_mech(0, 0, 0);
  observer.sensors[1] = 4;
  calculate(&los_map, &map, &observer, 0, 0, 5, 2);

  /* Unlit seestate < 0 handling at map_los.c:589-590. */
  los_expect_true(state, "unlit hex beyond visual range is NOLOS",
                  !hex_has(&los_map, 4, 0, MAPLOSHEX_SEE));

  fire.x = 4;
  fire.y = 0;
  calculate(&los_map, &map, &observer, 0, 0, 5, 2);

  /* Lit seestate < 0 handling at map_los.c:589 and 631-639. */
  los_expect_true(state, "lit hex beyond visual range remains visible",
                  hex_has(&los_map, 4, 0, MAPLOSHEX_SEE));
}

int main(void) {
  LosTestState state = {0};
  test_map_hex_bounds(&state);
  test_clairvoyance(&state);
  test_terrainless_sensors(&state);
  test_size_guards(&state);
  test_flat_map_range(&state);
  test_ridge_occlusion(&state);
  test_woods_obstruction(&state);
  test_mountain_and_water_blocks(&state);
  test_sensor_specific_obstruction_limits(&state);
  test_water_world_setup(&state);
  test_litemark_map(&state);
  test_searchlight_tracing(&state);
  test_searchlight_unit_heights(&state);
  test_weather_sensor_ranges(&state);
  test_lit_out_of_range_hexes(&state);
  return los_test_result(&state);
}
