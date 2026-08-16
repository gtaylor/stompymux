#include "btech_los_test.h"

#include "btech/context.h"
#include "btech_event.h"
#include "btech/core/context_internal.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_contacts_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

struct Mech {
  BtechContext *context;
  BattleMap *map;
  UnitClass unit_class;
  MechMovementType movement;
  MechConditionSummary condition;
  int x;
  int y;
  int z;
  int elevation;
  int tonnage;
  float speed;
  float excess_heat;
  float heat_production;
  float heat_dissipation;
  char terrain;
  bool started;
  bool landed;
  bool jumping;
  bool fired;
  bool destroyed;
  bool out_of_control;
  bool jellied;
  bool towed;
  DbRef carried;
  bool carries_club;
  bool homing_beacon;
  int team;
  int startup_events;
  int stand_events;
  int changing_hulldown_events;
  int vehicle_burn_events;
};

static bool seismic_stopped;

BtechContext *mech_context(const Mech *mech) { return mech->context; }
DbRef mech_map_dbref(const Mech *mech [[maybe_unused]]) { return 1; }
BattleMap *btech_context_get_map(BtechContext *context [[maybe_unused]],
                                 DbRef dbref [[maybe_unused]]) {
  return nullptr;
}
UnitClass mech_class(const Mech *mech) { return mech->unit_class; }
MechMovementType mech_movement_type(const Mech *mech) { return mech->movement; }
MechConditionSummary mech_condition_summary(const Mech *mech) {
  return mech->condition;
}
bool mech_searchlight_active(const Mech *mech) {
  return mech->condition.searchlight_on;
}
int mech_position_z(const Mech *mech) { return mech->z; }
int mech_position_x(const Mech *mech) { return mech->x; }
int mech_position_y(const Mech *mech) { return mech->y; }
int mech_position_elevation_magnitude(const Mech *mech) {
  return mech->elevation;
}
char mech_real_terrain_get(Mech *mech) { return mech->terrain; }
int bridge_w_elevation(Mech *mech) { return mech->elevation; }
bool mech_is_any_ecm_disturbed(const Mech *mech) {
  return mech->condition.ecm_disturbed || mech->condition.angel_ecm_disturbed;
}
bool btech_context_seismic_detects_stopped_units(const BtechContext *context
                                                 [[maybe_unused]]) {
  return seismic_stopped;
}
bool mech_is_started(const Mech *mech) { return mech->started; }
bool mech_is_destroyed(const Mech *mech) { return mech->destroyed; }
bool mech_is_landed(const Mech *mech) { return mech->landed; }
bool mech_is_jumping(const Mech *mech) { return mech->jumping; }
bool mech_is_out_of_control(const Mech *mech) { return mech->out_of_control; }
bool mech_is_fallen(const Mech *mech) { return mech->condition.fallen; }
bool mech_is_jellied(const Mech *mech) { return mech->jellied; }
bool mech_has_fired_recently(const Mech *mech) { return mech->fired; }
float mech_current_speed(const Mech *mech) { return mech->speed; }
float mech_excess_heat(const Mech *mech) { return mech->excess_heat; }
float mech_heat_production(const Mech *mech) { return mech->heat_production; }
float mech_heat_dissipation(const Mech *mech) { return mech->heat_dissipation; }
int mech_tonnage(const Mech *mech) { return mech->tonnage; }
int mech_real_tonnage(const Mech *mech) { return mech->tonnage; }
int mech_team(const Mech *mech) { return mech->team; }
DbRef mech_carried_dbref(const Mech *mech) { return mech->carried; }
bool mech_is_towed(const Mech *mech) { return mech->towed; }
bool mech_section_carries_club(const Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]]) {
  return mech->carries_club;
}
bool mech_has_attached_homing_beacon(const Mech *mech) {
  return mech->homing_beacon;
}
int mech_event_count(const Mech *mech, MechEventType type) {
  if (type == EVENT_STARTUP)
    return mech->startup_events;
  if (type == EVENT_STAND)
    return mech->stand_events;
  if (type == EVENT_CHANGING_HULLDOWN)
    return mech->changing_hulldown_events;
  if (type == EVENT_VEHICLEBURN)
    return mech->vehicle_burn_events;
  return 0;
}
bool mech_is_flying_type(const Mech *mech) {
  return mech->movement == MOVE_FLY || mech->movement == MOVE_VTOL;
}
int m_number(Mech *mech [[maybe_unused]], int low, int high [[maybe_unused]]) {
  return low;
}
bool battle_map_sensor_is_disabled(const BattleMap *map, int sensor) {
  return (map->sensorflags & (1 << sensor)) != 0;
}
float mech_los_actual_elevation(BattleMap *map [[maybe_unused]],
                                int x [[maybe_unused]], int y [[maybe_unused]],
                                Mech *mech) {
  return (float)mech->z + 0.5F;
}

static Mech make_unit(BtechContext *context, BattleMap *map) {
  return (Mech){.context = context,
                .map = map,
                .unit_class = CLASS_MECH,
                .movement = MOVE_BIPED,
                .tonnage = 50,
                .terrain = GRASSLAND,
                .started = true,
                .landed = true};
}

static SensorContactRequest contact(Mech *observer, Mech *target,
                                    BattleMap *map, int flags, float range) {
  return (SensorContactRequest){.observer = observer,
                                .target = target,
                                .map = map,
                                .range = range,
                                .flags = flags};
}

static void test_visibility_ranges(LosTestState *state, Mech *target,
                                   BattleMap *map) {
  SensorVisibilityRequest request = {
      .target = target, .map = map, .range = 10.0F, .condition_range = 10};
  los_expect_true(state, "visual includes exact visibility boundary",
                  vislight_see(&request) > 0);
  request.range = 10.1F;
  los_expect_int(state, "visual rejects beyond visibility", 0,
                 vislight_see(&request));
  request.light = 0;
  request.range = 20.0F;
  los_expect_true(state, "light amplification doubles dark range",
                  liteamp_see(&request) > 0);
  request.light = 1;
  los_expect_int(state, "light amplification loses dark range in light", 0,
                 liteamp_see(&request));
  request.range = 8.0F;
  los_expect_int(state, "seismic range chance", 18, seismic_see(&request));
  request.range = 200.0F;
  los_expect_int(state, "radar chance has floor", 10, radar_see(&request));
  los_expect_int(state, "active probe is deterministic", 101,
                 bap_see(&request));

  target->unit_class = CLASS_BSUIT;
  request.range = 3.0F;
  request.condition_range = 30;
  int normal = vislight_see(&request);
  target->unit_class = CLASS_MECH;
  los_expect_int(state, "small visual targets divide detection chance",
                 vislight_see(&request) / 3, normal);
}

static void test_optical_and_em_contacts(LosTestState *state, Mech *observer,
                                         Mech *target, BattleMap *map) {
  SensorContactRequest request = contact(observer, target, map, 0, 3.0F);
  los_expect_true(state, "visual sees clear target", vislight_csee(&request));
  request.flags = BATTLE_MAP_LOS_SMOKE;
  los_expect_int(state, "smoke blocks visual", 0, vislight_csee(&request));
  request.flags = 3 * BATTLE_MAP_LOS_WOOD;
  los_expect_int(state, "three woods points block visual", 0,
                 vislight_csee(&request));
  request.flags = 2 * BATTLE_MAP_LOS_WOOD;
  los_expect_int(state, "two woods points block light amp", 0,
                 liteamp_csee(&request));
  request.flags = 0;
  target->condition.illuminated = true;
  los_expect_int(state, "illumination blinds light amp", 0,
                 liteamp_csee(&request));
  target->condition.illuminated = false;
  target->unit_class = CLASS_MW;
  los_expect_int(state, "infrared excludes dismounted pilots", 0,
                 infrared_csee(&request));
  los_expect_int(state, "electromagnetic excludes dismounted pilots", 0,
                 electrom_csee(&request));
  target->unit_class = CLASS_MECH;
  observer->condition.ecm_disturbed = true;
  los_expect_int(state, "ECM disturbance blocks electromagnetic", 0,
                 electrom_csee(&request));
  observer->condition.ecm_disturbed = false;
  request.flags = BATTLE_MAP_LOS_MOUNTAIN;
  los_expect_int(state, "mountain flag blocks electromagnetic", 0,
                 electrom_csee(&request));
}

static void test_seismic(LosTestState *state,
                         BtechContext *context [[maybe_unused]], Mech *observer,
                         Mech *target, BattleMap *map) {
  SensorContactRequest request = contact(observer, target, map, 0, 3.0F);
  target->speed = 11.0F;
  los_expect_true(state, "seismic sees moving ground unit",
                  seismic_csee(&request));
  target->speed = 0.0F;
  los_expect_int(state, "seismic rejects stopped unit", 0,
                 seismic_csee(&request));
  seismic_stopped = true;
  los_expect_true(state, "seismic stopped-unit configuration",
                  seismic_csee(&request));
  seismic_stopped = false;
  target->speed = 11.0F;
  target->jumping = true;
  los_expect_int(state, "seismic rejects jumping target", 0,
                 seismic_csee(&request));
  target->jumping = false;
  target->movement = MOVE_HOVER;
  los_expect_int(state, "seismic rejects hover target", 0,
                 seismic_csee(&request));
  target->movement = MOVE_BIPED;
  observer->jumping = true;
  los_expect_int(state, "jumping observer cannot use seismic", 0,
                 seismic_csee(&request));
  observer->jumping = false;
}

static void test_radar_and_probes(LosTestState *state, Mech *observer,
                                  Mech *target, BattleMap *map) {
  SensorContactRequest request = contact(observer, target, map, 0, 3.0F);
  target->z = 3;
  target->elevation = 0;
  los_expect_true(state, "radar sees target clear of surface",
                  radar_csee(&request));
  target->z = 2;
  los_expect_int(state, "radar rejects low target", 0, radar_csee(&request));
  target->z = 4;
  target->elevation = 3;
  los_expect_int(state, "radar rejects target hugging surface", 0,
                 radar_csee(&request));
  target->elevation = 0;
  target->z = 3;
  request.range = 9.0F;
  los_expect_int(state, "radar uses strict altitude-squared range", 0,
                 radar_csee(&request));

  request.range = 2.0F;
  target->condition.stealth_armor_active = true;
  los_expect_int(state, "stealth armor blocks Beagle", 0, bap_csee(&request));
  los_expect_true(state, "stealth armor does not block Bloodhound",
                  blood_csee(&request));
  target->condition.stealth_armor_active = false;
  target->condition.null_signature_active = true;
  los_expect_int(state, "null signature blocks Beagle", 0, bap_csee(&request));
  los_expect_true(state, "null signature does not block Bloodhound",
                  blood_csee(&request));
  target->condition.null_signature_active = false;
  target->condition.angel_ecm_protected = true;
  los_expect_int(state, "Angel ECM blocks Beagle", 0, bap_csee(&request));
  los_expect_int(state, "Angel ECM blocks Bloodhound", 0, blood_csee(&request));
}

static void test_to_hit_signatures(LosTestState *state, Mech *observer,
                                   Mech *target, BattleMap *map) {
  SensorToHitRequest request = {
      .observer = observer, .target = target, .map = map, .light = 2};
  target->excess_heat = 0.0F;
  target->heat_production = 0.0F;
  target->heat_dissipation = 0.0F;
  int cold = infrared_tohit(&request);
  target->excess_heat = 20.0F;
  target->heat_production = 30.0F;
  los_expect_true(state, "hot target improves infrared to-hit",
                  infrared_tohit(&request) < cold);
  target->heat_production = 0.0F;
  los_expect_int(state, "infrared ignores accumulated excess heat", cold,
                 infrared_tohit(&request));

  target->tonnage = 70;
  target->speed = 11.0F;
  target->fired = true;
  los_expect_int(state, "EM combines heavy, fast, firing signature", -1,
                 electrom_tohit(&request));
  los_expect_int(state, "seismic rewards heavy moving target", 0,
                 seismic_tohit(&request));
  request.flags = BATTLE_MAP_LOS_PARTIAL_COVER;
  target->condition.hull_down = true;
  los_expect_int(state, "visual hull-down partial cover", 5,
                 vislight_tohit(&request));
}

static void test_contact_status_format(LosTestState *state, Mech *target) {
  target->destroyed = true;
  target->startup_events = 1;
  target->condition.hull_down = true;
  target->towed = true;
  target->jumping = true;
  target->out_of_control = true;
  target->excess_heat = 1.0F;
  target->jellied = true;
  target->condition.searchlight_on = true;
  target->condition.illuminated = true;
  target->condition.swarm_target = 1;
  target->carries_club = true;
  target->homing_beacon = true;
  target->condition.eccm_enabled = true;
  target->condition.ecm_active = true;
  target->condition.ecm_protected = true;
  target->condition.ecm_disturbed = true;
  target->condition.spinning = true;

  MechStatusString status = mech_status_string(target, 1);
  los_expect_string(state, "contact status format", "DsHTJO+ILlWCNPEpeX",
                    status.text);
}

int main(void) {
  LosTestState state = {0};
  BtechContext context = {0};
  BattleMap map = {};
  Mech observer = make_unit(&context, &map);
  Mech target = make_unit(&context, &map);
  test_visibility_ranges(&state, &target, &map);
  test_optical_and_em_contacts(&state, &observer, &target, &map);
  test_seismic(&state, &context, &observer, &target, &map);
  test_radar_and_probes(&state, &observer, &target, &map);
  target = make_unit(&context, &map);
  test_to_hit_signatures(&state, &observer, &target, &map);
  target = make_unit(&context, &map);
  test_contact_status_format(&state, &target);
  return los_test_result(&state);
}
