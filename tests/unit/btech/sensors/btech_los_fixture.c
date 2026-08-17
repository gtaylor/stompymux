#include "btech_los_fixture.h"

#include "btech_channel.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_los.h"
#include "map_los_types.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_sensor.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

#include <math.h>

static char terrain[LOS_FIXTURE_HEIGHT][LOS_FIXTURE_WIDTH];
static char real_terrain[LOS_FIXTURE_HEIGHT][LOS_FIXTURE_WIDTH];
static char elevation[LOS_FIXTURE_HEIGHT][LOS_FIXTURE_WIDTH];
static Mech *map_units[BATTLE_MAP_UNIT_CAPACITY];
static int map_unit_count;
static int channel_error_count;
static BattleMap *fixture_map;
static int sensor_can_see_result;
static MechSensorObservationRequest sensor_observation;
static SensorDefinition sensors[NUM_SENSORS];
static int sensor_wood_limits[NUM_SENSORS];
static int sensor_water_limits[NUM_SENSORS];
static bool sensor_sees_mountains[NUM_SENSORS];

static constexpr float ALPHA = 93.09773F;
static constexpr float FULL_Y = (float)SCALEMAP;
static constexpr float HALF_Y = 0.5F * FULL_Y;

static SensorDefinition *sensor_at(int sensor) {
  return checked_storage_at(sensors, NUM_SENSORS, sizeof(*sensors),
                            (size_t)sensor);
}

static int *sensor_wood_limit_at(int sensor) {
  return checked_storage_at(sensor_wood_limits, NUM_SENSORS,
                            sizeof(*sensor_wood_limits), (size_t)sensor);
}

static int *sensor_water_limit_at(int sensor) {
  return checked_storage_at(sensor_water_limits, NUM_SENSORS,
                            sizeof(*sensor_water_limits), (size_t)sensor);
}

static bool *sensor_mountain_at(int sensor) {
  return checked_storage_at(sensor_sees_mountains, NUM_SENSORS,
                            sizeof(*sensor_sees_mountains), (size_t)sensor);
}

static Mech **map_unit_at(int index) {
  return checked_storage_at(map_units, BATTLE_MAP_UNIT_CAPACITY,
                            sizeof(*map_units), (size_t)index);
}

static bool fixture_sensor_can_see(const SensorContactRequest *request,
                                   int sensor) {
  if ((request->flags & BATTLE_MAP_LOS_MOUNTAIN) &&
      !*sensor_mountain_at(sensor))
    return false;
  if (battle_map_los_water_count(request->flags) >
      *sensor_water_limit_at(sensor))
    return false;
  if (battle_map_los_wood_count(request->flags) > *sensor_wood_limit_at(sensor))
    return false;
  return true;
}

static bool fixture_sensor_0_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 0);
}

static bool fixture_sensor_1_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 1);
}

static bool fixture_sensor_2_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 2);
}

static bool fixture_sensor_3_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 3);
}

static bool fixture_sensor_4_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 4);
}

static bool fixture_sensor_5_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 5);
}

static bool fixture_sensor_6_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 6);
}

static bool fixture_sensor_7_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 7);
}

static bool fixture_sensor_8_can_see(const SensorContactRequest *request) {
  return fixture_sensor_can_see(request, 8);
}

typedef bool (*SensorCanSeeCallback)(const SensorContactRequest *request);

static const SensorCanSeeCallback SENSOR_CAN_SEE_CALLBACKS[NUM_SENSORS] = {
    fixture_sensor_0_can_see, fixture_sensor_1_can_see,
    fixture_sensor_2_can_see, fixture_sensor_3_can_see,
    fixture_sensor_4_can_see, fixture_sensor_5_can_see,
    fixture_sensor_6_can_see, fixture_sensor_7_can_see,
    fixture_sensor_8_can_see,
};

static SensorCanSeeCallback sensor_can_see_callback_at(int sensor) {
  const SensorCanSeeCallback *callback = checked_storage_at_const(
      SENSOR_CAN_SEE_CALLBACKS, NUM_SENSORS, sizeof(*SENSOR_CAN_SEE_CALLBACKS),
      (size_t)sensor);
  return *callback;
}

static void reset_sensors(void) {
  for (int sensor = 0; sensor < NUM_SENSORS; ++sensor) {
    *sensor_at(sensor) =
        (SensorDefinition){.full_vision = 1,
                           .maximum_visibility = 60,
                           .can_see = sensor_can_see_callback_at(sensor)};
    *sensor_wood_limit_at(sensor) = 99;
    *sensor_water_limit_at(sensor) = 99;
    *sensor_mountain_at(sensor) = true;
  }
}

static char *cell(char values[LOS_FIXTURE_HEIGHT][LOS_FIXTURE_WIDTH], int x,
                  int y) {
  char (*row)[LOS_FIXTURE_WIDTH] = checked_storage_at(
      values, LOS_FIXTURE_HEIGHT, sizeof(*values), (size_t)y);
  return checked_storage_at(*row, LOS_FIXTURE_WIDTH, sizeof(**row), (size_t)x);
}

int battle_map_width(const BattleMap *map) { return map->map_width; }
int battle_map_height(const BattleMap *map) { return map->map_height; }
int battle_map_maximum_visibility(const BattleMap *map) { return map->maxvis; }
int battle_map_light(const BattleMap *map) { return map->maplight; }
int battle_map_visibility(const BattleMap *map) { return map->mapvis; }
int battle_map_cloud_base(const BattleMap *map) { return map->cloudbase; }

int battle_map_unit_count(const BattleMap *map [[maybe_unused]]) {
  return map_unit_count;
}

DbRef battle_map_unit_dbref(const BattleMap *map [[maybe_unused]], int index) {
  if (index < 0 || index >= map_unit_count || !*map_unit_at(index))
    return -1;
  return (*map_unit_at(index))->dbref;
}

bool battle_map_coordinate_is_valid(const BattleMap *map, int x, int y) {
  return x >= 0 && y >= 0 && x < map->map_width && y < map->map_height;
}

char map_elevation_get(const BattleMap *map [[maybe_unused]], int x, int y) {
  return *cell(elevation, x, y);
}

int battle_map_hex_elevation(BattleMap *map [[maybe_unused]], int x, int y) {
  const int value = *cell(elevation, x, y);
  const char value_terrain = *cell(real_terrain, x, y);
  return value_terrain == WATER || value_terrain == ICE ? -value : value;
}

char map_terrain_get(const BattleMap *map [[maybe_unused]], int x, int y) {
  return *cell(terrain, x, y);
}

char map_real_terrain_get(BattleMap *map [[maybe_unused]], int x, int y) {
  return *cell(real_terrain, x, y);
}

int mech_position_x(const Mech *mech) { return mech->x; }
int mech_position_y(const Mech *mech) { return mech->y; }
int mech_position_z(const Mech *mech) { return mech->z; }
int mech_technology_flags(const Mech *mech) { return mech->technology; }
UnitClass mech_class(const Mech *mech) { return mech->unit_class; }
MechMovementType mech_movement_type(const Mech *mech) { return mech->movement; }
bool mech_is_fallen(const Mech *mech) { return mech->fallen; }
bool mech_is_dropship(const Mech *mech) { return mech->dropship; }
char mech_real_terrain_get(Mech *mech) { return mech->terrain; }
MechConditionSummary mech_condition_summary(const Mech *mech) {
  return mech->condition;
}

int mech_sensor_index(const Mech *mech, int slot) {
  if (slot == 0)
    return mech->sensors[0];
  return mech->sensors[1];
}
bool mech_is_jellied(const Mech *mech) { return mech->jellied; }
bool mech_searchlight_active(const Mech *mech) { return mech->searchlight; }
bool mech_is_clairvoyant(const Mech *mech) { return mech->clairvoyant; }
DbRef mech_dbref(const Mech *mech) { return mech->dbref; }
BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return (BtechContext *)fixture_map;
}
DbRef mech_map_dbref(const Mech *mech [[maybe_unused]]) { return 1; }
void mech_map_dbref_set(Mech *mech [[maybe_unused]],
                        DbRef map_dbref [[maybe_unused]]) {}
int mech_map_slot(const Mech *mech) { return mech->map_slot; }

void mech_partial_cover_set(Mech *mech, bool covered) {
  mech->partial_cover = covered;
}

int mech_sensor_to_hit_bonus(const MechSensorToHitRequest *request) {
  return request->ammunition_mode;
}

int mech_sensor_can_see(const MechSensorObservationRequest *request
                        [[maybe_unused]]) {
  sensor_observation = *request;
  return sensor_can_see_result;
}

static void fixture_map_coord_to_real_coord(int hex_x, int hex_y, float *cart_x,
                                            float *cart_y) {
  *cart_x = (2.0F + (3.0F * (float)hex_x)) * ALPHA;
  *cart_y = ((hex_x % 2) ? 0.0F : HALF_Y) + ((float)hex_y * FULL_Y);
}

float mech_position_real_x(const Mech *mech) {
  float cart_x;
  float cart_y;
  fixture_map_coord_to_real_coord(mech->x, mech->y, &cart_x, &cart_y);
  return cart_x;
}

float mech_position_real_y(const Mech *mech) {
  float cart_x;
  float cart_y;
  fixture_map_coord_to_real_coord(mech->x, mech->y, &cart_x, &cart_y);
  return cart_y;
}
float mech_position_real_z(const Mech *mech) { return (float)mech->z * ZSCALE; }

const SensorDefinition *mech_sensor_definition(int sensor) {
  return sensor_at(sensor);
}

int in_weapon_arc(Mech *mech, float x [[maybe_unused]],
                  float y [[maybe_unused]]) {
  return mech->weapon_arc;
}

void map_coord_to_real_coord(int hex_x, int hex_y, float *cart_x,
                             float *cart_y) {
  fixture_map_coord_to_real_coord(hex_x, hex_y, cart_x, cart_y);
}

float map_spatial_range(const MapSpatialSegment *segment) {
  const float delta_x = segment->end.x - segment->start.x;
  const float delta_y = segment->end.y - segment->start.y;
  const float delta_z = segment->end.z - segment->start.z;
  return sqrtf((delta_x * delta_x) + (delta_y * delta_y) +
               (delta_z * delta_z)) /
         (float)SCALEMAP;
}

void visit_neighbor_hexes(BattleMap *map, int x, int y,
                          NeighborHexCallback callback, void *context) {
  const MapHexPosition offsets[] = {{.x = 0, .y = -1}, {.x = 1, .y = 0},
                                    {.x = 1, .y = 1},  {.x = 0, .y = 1},
                                    {.x = -1, .y = 1}, {.x = -1, .y = 0}};
  for (size_t index = 0; index < sizeof(offsets) / sizeof(*offsets); ++index) {
    const MapHexPosition *offset = checked_storage_at_const(
        offsets, sizeof(offsets) / sizeof(*offsets), sizeof(*offsets), index);
    const int neighbor_x = x + offset->x;
    int neighbor_y = y + offset->y;
    if (x % 2 && !(neighbor_x % 2))
      --neighbor_y;
    if (battle_map_coordinate_is_valid(map, neighbor_x, neighbor_y))
      callback(map, neighbor_x, neighbor_y, context);
  }
}

MapObject *first_mapobj(BattleMap *map, int type) {
  MapObject **objects = map->map_object;
  MapObject **slot = checked_storage_at(objects, NUM_MAPOBJTYPES,
                                        sizeof(*objects), (size_t)type);
  return *slot;
}

MapObject *next_mapobj(MapObject *object) { return object->next; }

Mech *btech_context_get_mech(BtechContext *context [[maybe_unused]],
                             DbRef dbref) {
  for (int i = 0; i < map_unit_count; ++i) {
    Mech *unit = *map_unit_at(i);
    if (unit && unit->dbref == dbref)
      return unit;
  }
  return nullptr;
}

BattleMap *btech_context_get_map(BtechContext *context [[maybe_unused]],
                                 DbRef dbref [[maybe_unused]]) {
  return fixture_map;
}

void btech_channel_send(BtechContext *context [[maybe_unused]],
                        BtechChannel channel,
                        const char *format [[maybe_unused]], ...) {
  if (channel == BTECH_CHANNEL_MECH_ERRORS)
    ++channel_error_count;
}

void mech_notify(Mech *mech [[maybe_unused]],
                 MechNotifyAudience audience [[maybe_unused]],
                 const char *buffer [[maybe_unused]]) {}

int bounded(int lower, int value, int upper) {
  if (value < lower)
    return lower;
  return value > upper ? upper : value;
}

int max(int first, int second) { return first > second ? first : second; }

void los_fixture_reset(BattleMap *map) {
  *map = (BattleMap){.map_width = LOS_FIXTURE_WIDTH,
                     .map_height = LOS_FIXTURE_HEIGHT,
                     .mapvis = 60,
                     .maplight = 1,
                     .maxvis = 60};
  map_unit_count = 0;
  channel_error_count = 0;
  fixture_map = map;
  sensor_can_see_result = 1;
  sensor_observation = (MechSensorObservationRequest){0};
  reset_sensors();
  for (int y = 0; y < LOS_FIXTURE_HEIGHT; ++y) {
    for (int x = 0; x < LOS_FIXTURE_WIDTH; ++x)
      los_fixture_set_hex(x, y, GRASSLAND, GRASSLAND, 0);
  }
}

void los_fixture_set_hex(int x, int y, char new_terrain, char new_real_terrain,
                         int new_elevation) {
  *cell(terrain, x, y) = new_terrain;
  *cell(real_terrain, x, y) = new_real_terrain;
  *cell(elevation, x, y) = (char)new_elevation;
}

void los_fixture_fill_line(int x, int y_start, int y_end, char new_terrain,
                           char new_real_terrain, int new_elevation) {
  const int increment = y_start <= y_end ? 1 : -1;
  for (int y = y_start;; y += increment) {
    los_fixture_set_hex(x, y, new_terrain, new_real_terrain, new_elevation);
    if (y == y_end)
      return;
  }
}

Mech los_fixture_make_mech(int x, int y, int z) {
  return (Mech){.x = x,
                .y = y,
                .z = z,
                .dbref = -1,
                .sensors = {0, 1},
                .weapon_arc = FORWARDARC,
                .unit_class = CLASS_MECH,
                .movement = MOVE_BIPED,
                .terrain = GRASSLAND};
}

void los_fixture_map_unit_set(int index, Mech *unit) {
  Mech **slot = map_unit_at(index);
  *slot = unit;
  if (index >= map_unit_count)
    map_unit_count = index + 1;
}

int los_fixture_channel_error_count(void) { return channel_error_count; }

void los_fixture_sensor_can_see_result_set(int result) {
  sensor_can_see_result = result;
}

int los_fixture_sensor_observation_visibility(void) {
  return sensor_observation.map_visibility;
}

int los_fixture_sensor_observation_light(void) {
  return sensor_observation.map_light;
}

int los_fixture_sensor_observation_cloud_base(void) {
  return sensor_observation.cloud_base;
}

void los_fixture_sensor_set(int sensor, int maximum_visibility,
                            bool full_vision, int wood_limit, int water_limit,
                            bool sees_mountains) {
  SensorDefinition *definition = sensor_at(sensor);
  definition->maximum_visibility = maximum_visibility;
  definition->full_vision = full_vision ? 1 : 0;
  int *wood_slot = sensor_wood_limit_at(sensor);
  *wood_slot = wood_limit;
  int *water_slot = sensor_water_limit_at(sensor);
  *water_slot = water_limit;
  bool *mountain_slot = sensor_mountain_at(sensor);
  *mountain_slot = sees_mountains;
}

int los_fixture_flags(BattleMap *map, Mech *observer, Mech *target) {
  return mech_los_calculate_flags(&(MechLosCalculation){
      .observer = observer,
      .target = target,
      .map = map,
      .target_hex = {.x = target->x, .y = target->y},
      .hex_range = 2.0F,
  });
}

int los_fixture_flags_to_hex(BattleMap *map, Mech *observer, int x, int y,
                             float range) {
  return mech_los_calculate_flags(&(MechLosCalculation){
      .observer = observer,
      .map = map,
      .target_hex = {.x = x, .y = y},
      .hex_range = range,
  });
}
