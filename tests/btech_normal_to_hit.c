#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_c3_misc_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_los_api.h"
#include "mech_network_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static Mech attacker;
static Mech target;
static Mech spotter;
static BtechContext *context = (BtechContext *)&attacker;
static int gunnery;
static int spotting;
static int base_modifier;
static int section_modifier;
static int attacker_movement;
static int target_movement;
static int heat_modifier;
static int fire_mode;
static int ammunition_mode;
static int technology_flags;
static int c3_size;
static float network_range;
static DbRef c3_reference;
static DbRef spotter_dbref;
static bool underwater;
static char attacker_terrain;
static int attacker_z;
static int terrain_modifier;
static WeaponRangeProfile ranges;
static int effective_range;
static int effective_water_range;
static bool extended_ranges;

int mech_critical_fire_mode(const Mech *mech, int section, int critical) {
  (void)mech;
  (void)section;
  (void)critical;
  return fire_mode;
}
int mech_critical_ammo_mode(const Mech *mech, int section, int critical) {
  (void)mech;
  (void)section;
  (void)critical;
  return ammunition_mode;
}
BtechContext *mech_context(const Mech *mech) {
  (void)mech;
  return context;
}
MechConditionSummary mech_condition_summary(const Mech *mech) {
  (void)mech;
  return (MechConditionSummary){0};
}
char mech_real_terrain_get(Mech *mech) {
  (void)mech;
  return attacker_terrain;
}
int mech_position_z(const Mech *mech) {
  (void)mech;
  return attacker_z;
}
int FindPilotGunnery(Mech *mech, int weapon_index) {
  (void)mech;
  (void)weapon_index;
  return gunnery;
}
DbRef mech_spotter_dbref(const Mech *mech) {
  (void)mech;
  return spotter_dbref;
}
Mech *btech_context_get_mech(BtechContext *value, DbRef dbref) {
  (void)value;
  return dbref == 20 ? &spotter : nullptr;
}
void mech_notify(Mech *mech, MechNotifyAudience audience, const char *message) {
  (void)mech;
  (void)audience;
  (void)message;
}
int FindPilotSpotting(Mech *mech) {
  (void)mech;
  return spotting;
}
bool mech_is_aerospace_unit(const Mech *mech) {
  (void)mech;
  return false;
}
bool mech_is_landed(const Mech *mech) {
  (void)mech;
  return true;
}
UnitClass mech_class(const Mech *mech) {
  (void)mech;
  return CLASS_MECH;
}
int mech_base_to_hit_modifier(const Mech *mech) {
  (void)mech;
  return base_modifier;
}
int mech_section_base_to_hit(const Mech *mech, int section) {
  (void)mech;
  (void)section;
  return section_modifier;
}
WeaponRangeProfile weapon_catalogue_ranges(int weapon_index) {
  (void)weapon_index;
  return ranges;
}
bool btech_context_uses_extended_weapon_ranges(const BtechContext *value) {
  (void)value;
  return extended_ranges;
}
int weapon_catalogue_effective_range(int weapon_index, bool extended) {
  (void)weapon_index;
  return effective_range + (extended ? 5 : 0);
}
bool weapon_catalogue_is_hot_loaded(int weapon_index, int mode) {
  (void)weapon_index;
  (void)mode;
  return false;
}
bool btech_context_hotload_uses_half_modifier(const BtechContext *value) {
  (void)value;
  return false;
}
int mech_technology_flags(const Mech *mech) {
  (void)mech;
  return technology_flags;
}
bool mech_is_any_ecm_disturbed(const Mech *mech) {
  (void)mech;
  return false;
}
int mech_c3_network_size(const Mech *mech) {
  (void)mech;
  return c3_size;
}
float mech_network_range(Mech *mech, Mech *target_mech, float real_range,
                         DbRef *reference, bool is_c3) {
  (void)mech;
  (void)target_mech;
  (void)real_range;
  (void)is_c3;
  *reference = c3_reference;
  return network_range;
}
int mech_technology_flags_secondary(const Mech *mech) {
  (void)mech;
  return 0;
}
int mech_c3i_network_size(const Mech *mech) {
  (void)mech;
  return 0;
}
TargetingComputerType mech_targeting_computer_type(const Mech *mech) {
  (void)mech;
  return TARGCOMP_NORMAL;
}
bool weapon_catalogue_is_personal_combat(int weapon_index) {
  (void)weapon_index;
  return false;
}
bool mech_section_is_underwater(const Mech *mech, int section) {
  (void)mech;
  (void)section;
  return underwater;
}
int mech_infantry_technology_flags(const Mech *mech) {
  (void)mech;
  return 0;
}
bool mech_section_configuration_has(const Mech *mech, int section,
                                    int configuration) {
  (void)mech;
  (void)section;
  (void)configuration;
  return false;
}
int mech_attacker_movement_modifier(Mech *mech) {
  (void)mech;
  return attacker_movement;
}
int mech_overheat_to_hit_modifier(const Mech *mech) {
  (void)mech;
  return heat_modifier;
}
bool mech_has_section_special(const Mech *mech, int special) {
  (void)mech;
  (void)special;
  return false;
}
bool weapon_catalogue_is_narc(int weapon_index) {
  (void)weapon_index;
  return false;
}
bool weapon_catalogue_is_pulse(int weapon_index) {
  (void)weapon_index;
  return false;
}
bool weapon_catalogue_is_mrm(int weapon_index) {
  (void)weapon_index;
  return false;
}
bool weapon_catalogue_is_heavy(int weapon_index) {
  (void)weapon_index;
  return false;
}
bool mech_is_flying_type(const Mech *mech) {
  (void)mech;
  return false;
}
bool mech_is_out_of_control(const Mech *mech) {
  (void)mech;
  return false;
}
bool mech_is_jumping(const Mech *mech) {
  (void)mech;
  return false;
}
bool weapon_catalogue_is_rocket(int weapon_index) {
  (void)weapon_index;
  return false;
}
float mech_current_speed(const Mech *mech) {
  (void)mech;
  return 0.0F;
}
float mech_vertical_speed(const Mech *mech) {
  (void)mech;
  return 0.0F;
}
bool btech_context_overrides_weapon_arcs(const BtechContext *value) {
  (void)value;
  return false;
}
DbRef mech_target_dbref(const Mech *mech) {
  (void)mech;
  return 11;
}
DbRef mech_dbref(const Mech *mech) { return mech == &target ? 11 : 10; }
int mech_event_count(const Mech *mech, MechEventType type) {
  (void)mech;
  (void)type;
  return 0;
}
MechTargetPositionResult mech_target_position(const Mech *mech) {
  (void)mech;
  return (MechTargetPositionResult){0};
}
int InWeaponArc(Mech *mech, float x, float y) {
  (void)mech;
  (void)x;
  (void)y;
  return FORWARDARC;
}
bool mech_targets_hex_or_building(const Mech *mech) {
  (void)mech;
  return false;
}
int mech_cocoon_integrity(const Mech *mech) {
  (void)mech;
  return 0;
}
bool btech_context_dig_bonus_requires_front(const BtechContext *value) {
  (void)value;
  return false;
}
int mech_hit_group(Mech *mech, Mech *target_mech) {
  (void)mech;
  (void)target_mech;
  return FRONT;
}
int btech_context_dig_bonus(const BtechContext *value) {
  (void)value;
  return 0;
}
bool mech_is_dropship(const Mech *mech) {
  (void)mech;
  return false;
}
bool weapon_catalogue_is_missile(int weapon_index) {
  (void)weapon_index;
  return true;
}
int mech_aim_section(const Mech *mech) {
  (void)mech;
  return NUM_SECTIONS;
}
bool mech_is_immobile(const Mech *mech) {
  (void)mech;
  return false;
}
int mech_target_movement_modifier(Mech *mech, Mech *target_mech, float range) {
  (void)mech;
  (void)target_mech;
  (void)range;
  return target_movement;
}
DbRef mech_tagged_by_dbref(const Mech *mech) {
  (void)mech;
  return NOTHING;
}
int mech_team(const Mech *mech) {
  (void)mech;
  return 1;
}
int mech_los_terrain_modifier(const MechLosTerrainRequest *request) {
  (void)request;
  return terrain_modifier;
}
bool btech_context_woods_modify_damage(const BtechContext *value) {
  (void)value;
  return false;
}
int mech_position_x(const Mech *mech) {
  (void)mech;
  return 0;
}
int mech_position_y(const Mech *mech) {
  (void)mech;
  return 0;
}
char map_real_terrain_get(BattleMap *map, int x, int y) {
  (void)map;
  (void)x;
  (void)y;
  return GRASSLAND;
}
char map_elevation_get(const BattleMap *map, int x, int y) {
  (void)map;
  (void)x;
  (void)y;
  return 0;
}
int btech_context_sprint_to_hit_modifier(const BtechContext *value) {
  (void)value;
  return 0;
}
int mech_weapon_critical_to_hit_modifier(
    const WeaponCriticalToHitRequest *request) {
  (void)request;
  return 0;
}
EvaluationContext *btech_context_evaluation(BtechContext *value) {
  return (EvaluationContext *)value;
}
DbRef mech_pilot_dbref(const Mech *mech) {
  (void)mech;
  return 10;
}
void notify_printf(EvaluationContext *evaluation, DbRef player,
                   const char *format, ...) {
  (void)evaluation;
  (void)player;
  (void)format;
}
char *tprintf(const char *format, ...) {
  (void)format;
  static char text[] = "trace";
  return text;
}
void btech_channel_send(BtechContext *value, BtechChannel channel,
                        const char *format, ...) {
  (void)value;
  (void)channel;
  (void)format;
}
int weapon_catalogue_effective_water_range(int weapon_index, bool extended) {
  (void)weapon_index;
  return effective_water_range + (extended ? 2 : 0);
}

static void reset_fixture(void) {
  memset(&attacker, 0, sizeof(attacker));
  memset(&target, 0, sizeof(target));
  memset(&spotter, 0, sizeof(spotter));
  gunnery = 4;
  spotting = 4;
  base_modifier = 0;
  section_modifier = 0;
  attacker_movement = 0;
  target_movement = 0;
  heat_modifier = 0;
  fire_mode = 0;
  ammunition_mode = 0;
  technology_flags = 0;
  c3_size = 0;
  network_range = 0.0F;
  c3_reference = NOTHING;
  spotter_dbref = NOTHING;
  underwater = false;
  attacker_terrain = GRASSLAND;
  attacker_z = 0;
  terrain_modifier = 0;
  ranges = (WeaponRangeProfile){.minimum = 0,
                                .short_range = 5,
                                .medium_range = 10,
                                .long_range = 15,
                                .water_minimum = 0,
                                .water_short_range = 3,
                                .water_medium_range = 6,
                                .water_long_range = 9};
  effective_range = 15;
  effective_water_range = 9;
  extended_ranges = false;
}

static MechNormalToHitResult calculate(float range, int indirect_fire) {
  return mech_normal_to_hit_calculate(&(MechNormalToHitRequest){
      .attacker = &attacker,
      .map = (BattleMap *)&attacker,
      .section = 1,
      .critical = 2,
      .weapon_index = 3,
      .range = range,
      .target = &target,
      .indirect_fire = indirect_fire,
  });
}

static int test_base_range_movement_and_heat(void) {
  reset_fixture();
  gunnery = 3;
  base_modifier = 1;
  section_modifier = 2;
  attacker_movement = 2;
  target_movement = 3;
  heat_modifier = 1;
  const int actual = calculate(8.0F, 1000).value;
  if (actual != 14)
    fprintf(stderr, "base/range/movement/heat: expected 14, got %d\n", actual);
  return actual != 14;
}

static int test_underwater_and_indirect(void) {
  reset_fixture();
  attacker_terrain = WATER;
  attacker_z = -1;
  underwater = true;
  const int underwater_actual = calculate(5.0F, 1000).value;
  if (underwater_actual != 7) {
    fprintf(stderr, "underwater: expected 7, got %d\n", underwater_actual);
    return 1;
  }

  reset_fixture();
  spotter_dbref = 20;
  spotting = 5;
  terrain_modifier = 4;
  const int indirect_actual = calculate(4.0F, 2).value;
  if (indirect_actual != 7)
    fprintf(stderr, "indirect: expected 7, got %d\n", indirect_actual);
  return indirect_actual != 7;
}

static int test_c3_range_reference(void) {
  reset_fixture();
  technology_flags = C3_SLAVE_TECH;
  c3_size = 1;
  network_range = 4.0F;
  c3_reference = 77;
  MechNormalToHitResult result = calculate(12.0F, 1000);
  if (result.value != 4 || result.c3_reference != 77)
    fprintf(stderr, "C3: expected {4,77}, got {%d,%ld}\n", result.value,
            result.c3_reference);
  return result.value != 4 || result.c3_reference != 77;
}

int main(void) {
  int failures = 0;
  if (test_base_range_movement_and_heat()) {
    fprintf(stderr, "base, range, movement, and heat cases failed\n");
    ++failures;
  }
  if (test_underwater_and_indirect()) {
    fprintf(stderr, "underwater and indirect-fire cases failed\n");
    ++failures;
  }
  if (test_c3_range_reference()) {
    fprintf(stderr, "C3 range-reference case failed\n");
    ++failures;
  }
  return failures != 0;
}
