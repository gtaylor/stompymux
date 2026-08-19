/*
 * Drives the production autopilot approach adapters against a real map.
 *
 * The runner hands this test a private copy of the game directory, and the
 * map is loaded from it with the production map_load(), so terrain reaches
 * autopilot_speed_up_for_target()/autopilot_slow_down_for_target() through
 * the same encoding and accessors the server uses. Only the Mech accessors
 * and the two movement commands the adapters call are harnessed, so what is
 * under test is the adapter wiring itself: which policy questions get asked,
 * which map lookups happen, and which commands come out.
 */

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ai_api.h"
#include "autopilot.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "context_internal.h"
#include "map.h"
#include "map_api.h"
#include "map_conditions_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

enum { WATER_X = 50, WATER_Y = 15, LAND_X = 0, LAND_Y = 0 };
constexpr float MAXIMUM_SPEED = 30.0F;

struct Mech {
  int x;
  int y;
  int heading;
  int desired_heading;
  float desired_speed;
};

static BattleMap *harness_map;
static int speed_command_count;
static float last_commanded_speed;
static int heading_command_count;
static char last_commanded_heading[32];

/* Harnessed Mech state and commands. */

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return harness_map->xcode.context;
}

DbRef mech_map_dbref(const Mech *mech [[maybe_unused]]) { return 1; }

int mech_position_x(const Mech *mech) { return mech->x; }

int mech_position_y(const Mech *mech) { return mech->y; }

int mech_heading_degrees(const Mech *mech) { return mech->heading; }

int mech_desired_heading_degrees(const Mech *mech) {
  return mech->desired_heading;
}

float mech_desired_speed(const Mech *mech) { return mech->desired_speed; }

float mech_effective_maximum_speed(Mech *mech [[maybe_unused]]) {
  return MAXIMUM_SPEED;
}

BattleMap *btech_context_get_map(BtechContext *context [[maybe_unused]],
                                 DbRef map_dbref [[maybe_unused]]) {
  return harness_map;
}

void ai_set_speed(Mech *mech [[maybe_unused]], Autopilot *a [[maybe_unused]],
                  float s) {
  speed_command_count++;
  last_commanded_speed = s;
}

void mech_heading(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                  char *buffer) {
  heading_command_count++;
  (void)snprintf(last_commanded_heading, sizeof(last_commanded_heading), "%s",
                 buffer);
}

/* Map-loading collaborators outside this test's subject. */

int bounded(int low, int value, int high);

int bounded(int low, int value, int high) {
  return value < low ? low : value > high ? high : value;
}

void del_mapobjs(BattleMap *map [[maybe_unused]]) {}

/* The fixture hexes carry no fire, smoke, or other decorations. */
char find_decorations(BattleMap *map [[maybe_unused]], int x [[maybe_unused]],
                      int y [[maybe_unused]]) {
  return 0;
}

void btech_channel_send(BtechContext *context [[maybe_unused]],
                        BtechChannel channel [[maybe_unused]],
                        const char *format, ...) {
  va_list arguments;
  va_start(arguments, format);
  (void)vfprintf(stderr, format, arguments);
  va_end(arguments);
  (void)fputc('\n', stderr);
}

static bool nearly_equal(float left, float right) {
  return fabsf(left - right) < 0.0001F;
}

static void reset_commands(void) {
  speed_command_count = 0;
  last_commanded_speed = -1.0F;
  heading_command_count = 0;
  last_commanded_heading[0] = '\0';
}

static BattleMap *load_fixture_map(const char *game_directory) {
  BtechContext *context = calloc(1, sizeof(*context));
  ServerConfiguration *configuration = calloc(1, sizeof(*configuration));
  BattleMap *map = calloc(1, sizeof(*map));
  if (context == nullptr || configuration == nullptr || map == nullptr)
    return nullptr;
  (void)snprintf(configuration->database.map_db,
                 sizeof(configuration->database.map_db), "%s/maps",
                 game_directory);
  context->configuration = configuration;
  map->xcode.context = context;
  map->mynum = 1;

  /*
   * map_load() populates hexes through the map's current dimensions and only
   * records the file's own at the end. Server maps carry dimensions from the
   * state they were saved with; a map allocated here has none, so it is sized
   * from the file before the load.
   */
  char mapname[] = "drowned.map";
  char path[256];
  (void)snprintf(path, sizeof(path), "%s/maps/%s", game_directory, mapname);
  FILE *file = fopen(path, "r");
  if (file == nullptr)
    return nullptr;
  int width = 0;
  int height = 0;
  const bool DIMENSIONS_READ = map_read_dimensions(file, &width, &height);
  if (fclose(file) != 0 || !DIMENSIONS_READ)
    return nullptr;
  map->map_width = clamp_int_to_short(width);
  map->map_height = clamp_int_to_short(height);

  if (map_checkmapfile(map, mapname) != 1 || map_load(map, mapname) != 0)
    return nullptr;
  return map;
}

/* An accelerating unit is throttled by the terrain it is standing in. */
static int test_cruise_uses_real_terrain(Autopilot *autopilot) {
  Mech mech = {.x = LAND_X, .y = LAND_Y, .heading = 90, .desired_speed = 0.0F};
  reset_commands();
  autopilot_speed_up_for_target(
      &(AutopilotApproachRequest){.autopilot = autopilot,
                                  .mech = &mech,
                                  .target = {.x = 10, .y = 10},
                                  .bearing = 90});
  if (speed_command_count != 1 ||
      !nearly_equal(last_commanded_speed, MAXIMUM_SPEED))
    return 1;

  mech.x = WATER_X;
  mech.y = WATER_Y;
  reset_commands();
  autopilot_speed_up_for_target(
      &(AutopilotApproachRequest){.autopilot = autopilot,
                                  .mech = &mech,
                                  .target = {.x = 10, .y = 10},
                                  .bearing = 90});
  if (speed_command_count != 1 ||
      !nearly_equal(last_commanded_speed, 2.0F * MAXIMUM_SPEED / 3.0F))
    return 2;

  /* Standing on the goal issues no command. */
  mech.x = LAND_X;
  mech.y = LAND_Y;
  reset_commands();
  autopilot_speed_up_for_target(
      &(AutopilotApproachRequest){.autopilot = autopilot,
                                  .mech = &mech,
                                  .target = {.x = LAND_X, .y = LAND_Y},
                                  .bearing = 90});
  if (speed_command_count != 0)
    return 3;
  /* Nor does a unit already up to speed; reverse counts by its magnitude. */
  mech.desired_speed = -4.0F;
  reset_commands();
  autopilot_speed_up_for_target(
      &(AutopilotApproachRequest){.autopilot = autopilot,
                                  .mech = &mech,
                                  .target = {.x = 10, .y = 10},
                                  .bearing = 90});
  return speed_command_count == 0 ? 0 : 4;
}

/*
 * A unit that is no longer standing on the map it is attached to must not
 * fault: the terrain lookup only happens once acceleration is decided, and
 * the map accessors abort on out-of-range coordinates.
 */
static int
test_cruise_skips_terrain_when_not_accelerating(Autopilot *autopilot) {
  Mech mech = {.x = harness_map->map_width + 10,
               .y = harness_map->map_height + 10,
               .heading = 180,
               .desired_speed = 0.0F};
  reset_commands();
  autopilot_speed_up_for_target(
      &(AutopilotApproachRequest){.autopilot = autopilot,
                                  .mech = &mech,
                                  .target = {.x = 10, .y = 10},
                                  .bearing = 0});
  return speed_command_count == 0 ? 0 : 1;
}

static int test_approach_commands(Autopilot *autopilot) {
  Mech mech = {.x = LAND_X, .y = LAND_Y, .heading = 90, .desired_speed = 0.0F};

  /* Far from the goal the adapter declines to take over. */
  reset_commands();
  if (autopilot_slow_down_for_target(
          &(AutopilotApproachRequest){.autopilot = autopilot,
                                      .mech = &mech,
                                      .target = {.x = 10, .y = 10},
                                      .bearing = 90,
                                      .range = 3.0F}) ||
      speed_command_count != 0 || heading_command_count != 0)
    return 1;

  /* Badly misaligned: stop and ask for the bearing. */
  mech.heading = 300;
  reset_commands();
  if (!autopilot_slow_down_for_target(
          &(AutopilotApproachRequest){.autopilot = autopilot,
                                      .mech = &mech,
                                      .target = {.x = 10, .y = 10},
                                      .bearing = 10,
                                      .range = 1.0F}) ||
      speed_command_count != 1 || !nearly_equal(last_commanded_speed, 0.0F) ||
      heading_command_count != 1 || strcmp(last_commanded_heading, "10") != 0)
    return 2;

  /*
   * Straddling north is a 20 degree error, not a 340 degree one, so the unit
   * keeps closing at the slowdown ratio instead of stopping to turn.
   */
  mech.heading = 350;
  reset_commands();
  if (!autopilot_slow_down_for_target(
          &(AutopilotApproachRequest){.autopilot = autopilot,
                                      .mech = &mech,
                                      .target = {.x = 10, .y = 10},
                                      .bearing = 10,
                                      .range = 1.0F}) ||
      speed_command_count != 1 ||
      !nearly_equal(last_commanded_speed, 0.9F * MAXIMUM_SPEED) ||
      heading_command_count != 0)
    return 3;

  /* Standing on the goal and lined up: full stop, no turn. */
  reset_commands();
  if (!autopilot_slow_down_for_target(
          &(AutopilotApproachRequest){.autopilot = autopilot,
                                      .mech = &mech,
                                      .target = {.x = LAND_X, .y = LAND_Y},
                                      .bearing = 10,
                                      .range = 0.0F}) ||
      speed_command_count != 1 || !nearly_equal(last_commanded_speed, 0.0F) ||
      heading_command_count != 0)
    return 4;

  /* An already-correct desired heading is not re-issued. */
  mech.heading = 300;
  mech.desired_heading = 10;
  reset_commands();
  if (!autopilot_slow_down_for_target(
          &(AutopilotApproachRequest){.autopilot = autopilot,
                                      .mech = &mech,
                                      .target = {.x = 10, .y = 10},
                                      .bearing = 10,
                                      .range = 1.0F}) ||
      heading_command_count != 0)
    return 5;
  return 0;
}

int main(int argc, char *argv[]) {
  if (argc != 2) {
    (void)fprintf(stderr, "usage: autopilot_approach_integration_test "
                          "GAME_DIRECTORY\n");
    return 1;
  }
  const char *game_directory = *(const char *const *)checked_storage_at_const(
      argv, (size_t)argc, sizeof(*argv), 1);
  harness_map = load_fixture_map(game_directory);
  if (harness_map == nullptr) {
    (void)fprintf(stderr, "could not load the fixture map\n");
    return 2;
  }
  if (map_real_terrain_get(harness_map, WATER_X, WATER_Y) !=
          BATTLE_TERRAIN_WATER ||
      map_real_terrain_get(harness_map, LAND_X, LAND_Y) ==
          BATTLE_TERRAIN_WATER) {
    (void)fprintf(stderr, "the fixture map no longer has the expected hexes\n");
    return 3;
  }

  Autopilot autopilot = {};
  const int CRUISE = test_cruise_uses_real_terrain(&autopilot);
  if (CRUISE)
    return 10 + CRUISE;
  const int LAZY = test_cruise_skips_terrain_when_not_accelerating(&autopilot);
  if (LAZY)
    return 20 + LAZY;
  const int APPROACH = test_approach_commands(&autopilot);
  return APPROACH ? 30 + APPROACH : 0;
}
