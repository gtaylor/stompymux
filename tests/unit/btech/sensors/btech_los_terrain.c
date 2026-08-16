/*
 * mech_los_calculate_flags terrain scenarios, covering mech_los.c:
 * - bridge and water blocking behavior (211, 278)
 * - high-water and ice water accounting, including hex-target ice handling
 *   and submerged ice blocking (148, 251)
 * - submerged paths, sea floors, and water-air transitions (177, 211, 219)
 * - underwater range penalties (307-310) and water/woods saturation (301-304)
 * - AA-tech visibility for each endpoint (116-123)
 * - off-map hex targets and same-hex visibility (112, 181)
 * - water partial cover (295-296), final-hex terrain suppression (234-276),
 *   both-world movement modes (139-146), and high LOS terrain suppression
 *   (230)
 * - cached LOS wrapper results and terrain modifiers (314-348)
 * - LOS cache accessors and direct cache/hex check paths, including weather
 *   observation inputs (90-131, 353-412)
 */

#include "btech_los_fixture.h"
#include "btech_los_test.h"

#include "map_los_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "mech_los_api.h"
#include "mech_status_types.h"

static int flags_at_range(BattleMap *map, Mech *observer, Mech *target,
                          float range) {
  return mech_los_calculate_flags(&(MechLosCalculation){
      .observer = observer,
      .target = target,
      .map = map,
      .target_hex = {.x = target->x, .y = target->y},
      .hex_range = range,
  });
}

static void test_bridge_and_water_blocking(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, -2);
  Mech target = los_fixture_make_mech(2, 4, -2);
  observer.terrain = WATER;
  target.terrain = WATER;
  los_fixture_set_hex(2, 3, BRIDGE, BRIDGE, 0);
  los_fixture_set_hex(2, 4, WATER, WATER, 3);
  const int bridge_flags = los_fixture_flags(&map, &observer, &target);

  los_fixture_set_hex(2, 3, WATER, WATER, 0);
  const int water_flags = los_fixture_flags(&map, &observer, &target);

  /* Underwater sea-floor and bridge exceptions at mech_los.c:211. */
  los_expect_true(state,
                  "bridge stays transparent while water sea floor blocks",
                  !(bridge_flags & BATTLE_MAP_LOS_BLOCKED) &&
                      (water_flags & BATTLE_MAP_LOS_BLOCKED));
}

static void test_high_water_and_ice(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  los_fixture_set_hex(2, 3, HIGHWATER, HIGHWATER, 0);
  const int high_water_flags = los_fixture_flags(&map, &observer, &target);

  los_fixture_reset(&map);
  observer = los_fixture_make_mech(2, 2, -1);
  los_fixture_set_hex(2, 4, ICE, ICE, 0);
  const int ice_water_flags =
      los_fixture_flags_to_hex(&map, &observer, 2, 4, 2.0F);

  los_fixture_reset(&map);
  observer = los_fixture_make_mech(2, 2, -2);
  los_fixture_set_hex(2, 3, ICE, ICE, 0);
  los_fixture_set_hex(2, 4, ICE, ICE, 0);
  const int submerged_ice_flags =
      los_fixture_flags_to_hex(&map, &observer, 2, 4, 2.0F);

  /* High-water accounting at mech_los.c:247-249. */
  los_expect_int(state, "high water increments the water count", 1,
                 battle_map_los_water_count(high_water_flags));
  /* Ice hex-target elevation at mech_los.c:148,278. */
  los_expect_int(state, "ground observer sees an ice hex target",
                 BATTLE_MAP_LOS_TERRAIN_CALCULATED + BATTLE_MAP_LOS_WATER,
                 ice_water_flags);
  /* Submerged ice blocking at mech_los.c:251-254. */
  los_expect_true(state, "ice blocks when the sight line is below its surface",
                  submerged_ice_flags & BATTLE_MAP_LOS_BLOCKED);
}

static void test_submerged_paths(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, -2);
  Mech target = los_fixture_make_mech(2, 5, -2);
  observer.terrain = WATER;
  target.terrain = WATER;
  los_fixture_fill_line(2, 3, 5, WATER, WATER, 3);
  const int water_flags = los_fixture_flags(&map, &observer, &target);

  los_fixture_set_hex(2, 4, WATER, WATER, 0);
  const int floor_flags = los_fixture_flags(&map, &observer, &target);

  target = los_fixture_make_mech(2, 5, 0);
  target.terrain = WATER;
  const int water_air_flags = los_fixture_flags(&map, &observer, &target);

  /* Underwater water-path traversal at mech_los.c:211. */
  los_expect_true(state, "submerged observer sees through deep water",
                  !(water_flags & BATTLE_MAP_LOS_BLOCKED));
  /* Underwater sea-floor blocking at mech_los.c:211. */
  los_expect_true(state, "underwater sea floor blocks sight",
                  floor_flags & BATTLE_MAP_LOS_BLOCKED);
  /* Underwater water-air transition at mech_los.c:177,219. */
  los_expect_true(state, "submerged observer cannot see into air",
                  water_air_flags & BATTLE_MAP_LOS_BLOCKED);
}

static void test_underwater_penalties(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 0, -2);
  Mech target = los_fixture_make_mech(2, 4, -2);
  observer.terrain = WATER;
  target.terrain = WATER;
  los_fixture_fill_line(2, 1, 4, WATER, WATER, 3);
  const int mountain_flags = los_fixture_flags(&map, &observer, &target);

  target.y = 8;
  los_fixture_fill_line(2, 5, 8, WATER, WATER, 3);
  const int fire_flags = los_fixture_flags(&map, &observer, &target);

  /* First underwater range threshold at mech_los.c:307-308. */
  los_expect_true(state, "three underwater hexes set mountain",
                  mountain_flags & BATTLE_MAP_LOS_MOUNTAIN);
  /* Fire remains clear before the second underwater threshold at
   * mech_los.c:309. */
  los_expect_true(state, "three underwater hexes do not set fire",
                  !(mountain_flags & BATTLE_MAP_LOS_FIRE));
  /* Second underwater range threshold at mech_los.c:309-310. */
  los_expect_true(state, "seven underwater hexes set fire",
                  fire_flags & BATTLE_MAP_LOS_FIRE);
}

static void test_terrain_count_saturation(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 0, 0);
  Mech target = los_fixture_make_mech(2, 16, 0);
  los_fixture_fill_line(2, 1, 16, HEAVY_FOREST, HEAVY_FOREST, 0);
  const int woods_flags = los_fixture_flags(&map, &observer, &target);

  los_fixture_reset(&map);
  observer = los_fixture_make_mech(2, 0, 0);
  target = los_fixture_make_mech(2, 16, 0);
  los_fixture_fill_line(2, 1, 16, HIGHWATER, HIGHWATER, 0);
  const int water_flags = los_fixture_flags(&map, &observer, &target);

  /* Packed terrain count bounds at mech_los.c:301-304. */
  los_expect_true(state, "water and woods counts saturate below packed limits",
                  battle_map_los_wood_count(woods_flags) ==
                          BATTLE_MAP_LOS_MAX_WOOD - 1 &&
                      battle_map_los_water_count(water_flags) ==
                          BATTLE_MAP_LOS_MAX_WATER - 1);
}

static void test_aa_visibility(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  const int no_aa_flags = flags_at_range(&map, &observer, &target, 180.0F);
  observer.technology = AA_TECH;
  const int observer_aa_flags =
      flags_at_range(&map, &observer, &target, 180.0F);

  observer.technology = 0;
  target.technology = AA_TECH;
  const int target_aa_flags = flags_at_range(&map, &observer, &target, 180.0F);

  /* Non-AA maximum visibility at mech_los.c:116-123. */
  los_expect_true(state, "range 180 blocks without AA tech",
                  no_aa_flags & BATTLE_MAP_LOS_BLOCKED);
  /* Observer AA-tech maximum visibility at mech_los.c:116-123. */
  los_expect_true(state, "observer AA tech extends visibility to 180",
                  !(observer_aa_flags & BATTLE_MAP_LOS_BLOCKED));
  /* Target AA-tech maximum visibility at mech_los.c:116-123. */
  los_expect_true(state, "target AA tech extends visibility to 180",
                  !(target_aa_flags & BATTLE_MAP_LOS_BLOCKED));
}

static void test_hex_target_boundaries(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  const int off_map_flags =
      los_fixture_flags_to_hex(&map, &observer, LOS_FIXTURE_WIDTH, 2, 1.0F);
  const int same_hex_flags =
      los_fixture_flags_to_hex(&map, &observer, 2, 2, 1.0F);

  /* Off-map and same-hex shortcuts at mech_los.c:112 and 181. */
  los_expect_true(state, "off-map hex blocks while same hex remains visible",
                  (off_map_flags & BATTLE_MAP_LOS_BLOCKED) &&
                      !(same_hex_flags & BATTLE_MAP_LOS_BLOCKED));
}

static void test_water_partial_cover(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, -1);
  target.terrain = WATER;
  const int flags = los_fixture_flags(&map, &observer, &target);

  /* Water target partial cover at mech_los.c:295-296. */
  los_expect_true(state, "water target at depth one receives partial cover",
                  flags & BATTLE_MAP_LOS_PARTIAL_COVER);
}

static void test_final_hex_terrain_suppression(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  los_fixture_set_hex(2, 4, SMOKE, GRASSLAND, 0);
  const int smoke_flags = los_fixture_flags(&map, &observer, &target);

  los_fixture_set_hex(2, 4, FIRE, GRASSLAND, 0);
  const int fire_flags = los_fixture_flags(&map, &observer, &target);

  los_fixture_set_hex(2, 4, MOUNTAINS, MOUNTAINS, 0);
  const int mountain_flags = los_fixture_flags(&map, &observer, &target);

  /* Final smoke exclusion at mech_los.c:234-238. */
  los_expect_true(state, "final smoke hex is not an LOS smoke flag",
                  !(smoke_flags & BATTLE_MAP_LOS_SMOKE));
  /* Final fire exclusion at mech_los.c:239-242. */
  los_expect_true(state, "final fire hex is not an LOS fire flag",
                  !(fire_flags & BATTLE_MAP_LOS_FIRE));
  /* Final mountain exclusion at mech_los.c:271-274. */
  los_expect_true(state, "final mountain hex is not an LOS mountain flag",
                  !(mountain_flags & BATTLE_MAP_LOS_MOUNTAIN));
}

static void test_both_worlds_movement(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  observer.terrain = WATER;
  target.terrain = WATER;
  observer.movement = MOVE_HOVER;
  int hover_flags = los_fixture_flags(&map, &observer, &target);

  observer.movement = MOVE_HULL;
  int hull_flags = los_fixture_flags(&map, &observer, &target);

  observer.movement = MOVE_FOIL;
  int foil_flags = los_fixture_flags(&map, &observer, &target);

  /* Hover both-world classification at mech_los.c:139-146. */
  los_expect_true(state, "surface hover sees both worlds",
                  !(hover_flags & BATTLE_MAP_LOS_BLOCKED));
  /* Hull both-world classification at mech_los.c:139-146. */
  los_expect_true(state, "surface hull sees both worlds",
                  !(hull_flags & BATTLE_MAP_LOS_BLOCKED));
  /* Foil both-world classification at mech_los.c:139-146. */
  los_expect_true(state, "surface foil sees both worlds",
                  !(foil_flags & BATTLE_MAP_LOS_BLOCKED));
}

static void test_high_los_suppresses_terrain_counting(LosTestState *state) {
  BattleMap map;
  los_fixture_reset(&map);
  Mech observer = los_fixture_make_mech(2, 2, 1);
  Mech target = los_fixture_make_mech(2, 4, 1);
  los_fixture_set_hex(2, 3, HEAVY_FOREST, HEAVY_FOREST, 0);
  const int woods_flags = los_fixture_flags(&map, &observer, &target);

  los_fixture_set_hex(2, 3, HIGHWATER, HIGHWATER, 0);
  const int water_flags = los_fixture_flags(&map, &observer, &target);

  /* Height-gated terrain counting at mech_los.c:230. */
  los_expect_true(state, "LOS two levels above terrain ignores woods and water",
                  battle_map_los_wood_count(woods_flags) == 0 &&
                      battle_map_los_water_count(water_flags) == 0);
}

static void test_cached_los_wrappers(LosTestState *state) {
  BattleMap map;
  unsigned short observer_los[2] = {0};
  unsigned short target_los[2] = {0};
  unsigned short *los_rows[2] = {observer_los, target_los};
  los_fixture_reset(&map);
  map.dynamic_size = 2;
  map.lo_sinfo = los_rows;
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  observer.map_slot = 0;
  target.map_slot = 1;
  battle_map_los_flags_set(&map, observer.map_slot, target.map_slot,
                           BATTLE_MAP_LOS_SEEN_PRIMARY |
                               BATTLE_MAP_LOS_PARTIAL_COVER);
  const int modifier = mech_los_terrain_modifier(&(MechLosTerrainRequest){
      .observer = &observer,
      .target = &target,
      .map = &map,
      .ammunition_mode = 7,
  });

  /* Cached terrain modifier forwarding at mech_los.c:314-333. */
  los_expect_int(state, "terrain modifier returns the sensor bonus", 7,
                 modifier);
  los_expect_true(state, "terrain modifier applies cached partial cover",
                  target.partial_cover);
  /* Terrain modifier null-target shortcut at mech_los.c:334-335. */
  los_expect_int(state, "terrain modifier ignores a missing target", 0,
                 mech_los_terrain_modifier(&(MechLosTerrainRequest){
                     .observer = &observer,
                     .map = &map,
                 }));
  /* Self-LOS shortcut at mech_los.c:341-342. */
  los_expect_int(state, "self LOS is unblocked", 1,
                 mech_los_check_unblocked(&observer, &observer, 2, 2, 0.0F));
  /* Cached clear LOS result at mech_los.c:344-348. */
  los_expect_int(state, "cached visible target remains unblocked",
                 BATTLE_MAP_LOS_SEEN_PRIMARY,
                 mech_los_check_unblocked(&observer, &target, 2, 4, 2.0F));

  battle_map_los_flags_set(&map, observer.map_slot, target.map_slot,
                           BATTLE_MAP_LOS_SEEN_PRIMARY |
                               BATTLE_MAP_LOS_BLOCKED);
  /* Cached blocked LOS result at mech_los.c:344-348. */
  los_expect_int(state, "cached blocked target is rejected", 0,
                 mech_los_check_unblocked(&observer, &target, 2, 4, 2.0F));
}

static void test_los_cache_accessors(LosTestState *state) {
  BattleMap map;
  unsigned short observer_los[3] = {0};
  unsigned short target_los[3] = {0};
  unsigned short third_los[3] = {0};
  unsigned short *los_rows[3] = {observer_los, target_los, third_los};
  los_fixture_reset(&map);
  map.dynamic_size = 3;
  map.lo_sinfo = los_rows;
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  observer.map_slot = 0;
  target.map_slot = 1;
  const unsigned short FLAGS =
      BATTLE_MAP_LOS_SEEN | BATTLE_MAP_LOS_BLOCKED + BATTLE_MAP_LOS_WOOD * 3 +
                                BATTLE_MAP_LOS_WATER * 4;
  battle_map_los_flags_set(&map, observer.map_slot, target.map_slot, FLAGS);

  /* Cached seen flag accessor at map_los.c:90-95. */
  los_expect_true(state, "LOS accessor recognizes a seen target",
                  battle_map_unit_is_seen(&map, &observer, &target));
  /* Cached blocked flag accessor at map_los.c:97-102. */
  los_expect_true(state, "LOS accessor recognizes a blocked target",
                  battle_map_unit_los_is_blocked(&map, &observer, &target));
  /* Cached woods-count accessor at map_los.c:104-109. */
  los_expect_int(state, "LOS accessor reads cached woods", 3,
                 battle_map_unit_los_wood_count(&map, &observer, &target));
  /* Cached water-count accessor at map_los.c:111-116. */
  los_expect_int(state, "LOS accessor reads cached water", 4,
                 battle_map_unit_los_water_count(&map, &observer, &target));
  /* Cached flag read/write accessors at map_los.c:118-126. */
  los_expect_int(
      state, "LOS flag accessor returns the written flags", FLAGS,
      battle_map_los_flags(&map, observer.map_slot, target.map_slot));

  observer_los[0] = BATTLE_MAP_LOS_SEEN;
  observer_los[1] = BATTLE_MAP_LOS_BLOCKED;
  observer_los[2] = BATTLE_MAP_LOS_WOOD;
  map.first_free = 2;
  battle_map_los_observer_clear(&map, observer.map_slot);

  /* Observer cache clearing at map_los.c:128-131. */
  los_expect_int(state, "observer clear removes the first cached target", 0,
                 observer_los[0]);
  los_expect_int(state, "observer clear removes the last active target", 0,
                 observer_los[1]);
  los_expect_int(state, "observer clear preserves an inactive target slot",
                 BATTLE_MAP_LOS_WOOD, observer_los[2]);
}

static void test_mech_los_check_paths(LosTestState *state) {
  BattleMap map;
  unsigned short observer_los[2] = {0};
  unsigned short target_los[2] = {0};
  unsigned short *los_rows[2] = {observer_los, target_los};
  los_fixture_reset(&map);
  map.dynamic_size = 2;
  map.lo_sinfo = los_rows;
  Mech observer = los_fixture_make_mech(2, 2, 0);
  Mech target = los_fixture_make_mech(2, 4, 0);
  observer.map_slot = 0;
  target.map_slot = 1;
  observer.clairvoyant = true;

  /* Clairvoyant shortcut at mech_los.c:375-377. */
  los_expect_int(state, "clairvoyance bypasses LOS calculation", 1,
                 mech_los_check(&observer, nullptr, 2, 4, 2.0F));
  observer.clairvoyant = false;

  battle_map_los_flags_set(&map, observer.map_slot, target.map_slot,
                           BATTLE_MAP_LOS_SEEN_PRIMARY);
  /* Primary cached LOS result at mech_los.c:387-394. */
  los_expect_int(state, "primary cache hit returns primary visibility",
                 BATTLE_MAP_LOS_SEEN_PRIMARY,
                 mech_los_check(&observer, &target, 2, 4, 2.0F));
  battle_map_los_flags_set(&map, observer.map_slot, target.map_slot,
                           BATTLE_MAP_LOS_SEEN_SECONDARY |
                               BATTLE_MAP_LOS_BLOCKED);
  /* Secondary cached LOS result at mech_los.c:387-394. */
  los_expect_int(state, "secondary cache hit preserves blocked visibility",
                 BATTLE_MAP_LOS_SEEN_SECONDARY | BATTLE_MAP_LOS_BLOCKED,
                 mech_los_check(&observer, &target, 2, 4, 2.0F));
  battle_map_los_flags_set(&map, observer.map_slot, target.map_slot, 0);
  /* Cache miss at mech_los.c:387-395. */
  los_expect_int(state, "missing cache visibility returns zero", 0,
                 mech_los_check(&observer, &target, 2, 4, 2.0F));

  map.mapvis = 17;
  map.maplight = 4;
  map.cloudbase = 9;
  los_fixture_sensor_can_see_result_set(7);
  /* Hex-target sensor handoff at mech_los.c:396-412. */
  los_expect_int(state, "hex target returns the sensor result", 7,
                 mech_los_check(&observer, nullptr, 2, 4, 2.0F));
  los_expect_int(state, "hex target passes map visibility to the sensor", 17,
                 los_fixture_sensor_observation_visibility());
  los_expect_int(state, "hex target passes map light to the sensor", 4,
                 los_fixture_sensor_observation_light());
  los_expect_int(state, "hex target passes cloud base to the sensor", 9,
                 los_fixture_sensor_observation_cloud_base());
}

int main(void) {
  LosTestState state = {0};
  test_bridge_and_water_blocking(&state);
  test_high_water_and_ice(&state);
  test_submerged_paths(&state);
  test_underwater_penalties(&state);
  test_terrain_count_saturation(&state);
  test_aa_visibility(&state);
  test_hex_target_boundaries(&state);
  test_water_partial_cover(&state);
  test_final_hex_terrain_suppression(&state);
  test_both_worlds_movement(&state);
  test_high_los_suppresses_terrain_counting(&state);
  test_cached_los_wrappers(&state);
  test_los_cache_accessors(&state);
  test_mech_los_check_paths(&state);
  return los_test_result(&state);
}
