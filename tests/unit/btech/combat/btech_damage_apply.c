#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "environment_damage_api.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_damage_history_api.h"
#include "mech_equipment_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static Mech attacker;
static Mech target;
static BtechContext *context = (BtechContext *)&attacker;
static BattleMap *combat_map;
static bool combat_safe;
static int armor;
static int internal;
static int depleted_section;
static UnitClass fixture_class;
static int transfer_location;
static int armor_calls;
static int internal_calls;
static int notification_count;
static int attacker_damage;
static int target_damage;
static int turn_damage;
static int flood_count;
static int breach_checks;
static ArmorDamageRequest last_armor_request;
static InternalDamageRequest last_internal_request;

BtechContext *mech_context(const Mech *mech) {
  (void)mech;
  return context;
}

int btech_random_roll(BtechContext *value) {
  (void)value;
  return 12;
}

DbRef mech_map_dbref(const Mech *mech) {
  (void)mech;
  return 1;
}

BattleMap *btech_context_get_map(BtechContext *value, DbRef dbref) {
  (void)value;
  (void)dbref;
  return combat_map;
}

bool battle_map_is_combat_safe(const BattleMap *map) {
  (void)map;
  return combat_safe;
}

MechConditionSummary mech_condition_summary(const Mech *mech) {
  (void)mech;
  return (MechConditionSummary){0};
}

void mech_notify(Mech *mech, MechNotifyAudience audience, const char *message) {
  (void)mech;
  (void)audience;
  (void)message;
  ++notification_count;
}

int bsuit_swarmer_count(Mech *mech) {
  (void)mech;
  return 0;
}

DbRef mech_dbref(const Mech *mech) { return mech == &target ? 12 : 11; }

UnitClass mech_class(const Mech *mech) {
  (void)mech;
  return fixture_class;
}

bool mech_is_aerospace_unit(const Mech *mech) {
  (void)mech;
  return false;
}

int mech_section_internal(const Mech *mech, int section) {
  (void)mech;
  return section == depleted_section ? 0 : internal;
}

int mech_section_armor(const Mech *mech, int section) {
  (void)mech;
  (void)section;
  return armor;
}

int mech_hit_location_transfer(Mech *mech, int hit_location) {
  (void)mech;
  (void)hit_location;
  return transfer_location;
}

int mech_cocoon_integrity(const Mech *mech) {
  (void)mech;
  return 0;
}

void armor_string_from_index(int index, char *buffer, UnitClass unit_class,
                          MechMovementType movement_type) {
  (void)index;
  (void)unit_class;
  (void)movement_type;
  strcpy(buffer, "Front");
}

MechMovementType mech_movement_type(const Mech *mech) {
  (void)mech;
  return MOVE_TRACK;
}

void mech_damage_inflicted_add(Mech *mech, int damage) {
  (void)mech;
  attacker_damage += damage;
}

void mech_damage_taken_add(Mech *mech, int damage) {
  (void)mech;
  target_damage += damage;
}

void mech_printf(Mech *mech, MechNotifyAudience audience, const char *format,
                 ...) {
  (void)mech;
  (void)audience;
  (void)format;
  ++notification_count;
}

BtechDamageExperienceMode
btech_context_damage_experience_mode(const BtechContext *value) {
  (void)value;
  return BTECH_DAMAGE_XP_NONE;
}

int unit_damage_to_personal_combat(
    const PersonalCombatDamageConversion *conversion) {
  return conversion->damage;
}

int mech_technology_flags(const Mech *mech) {
  (void)mech;
  return 0;
}

int btech_context_stagger_mode(const BtechContext *value) {
  (void)value;
  return false;
}

void mech_turn_damage_add(Mech *mech, int damage) {
  (void)mech;
  turn_damage += damage;
}

int cause_armordamage(const ArmorDamageRequest *request) {
  ++armor_calls;
  last_armor_request = *request;
  return 0;
}

int cause_internaldamage(const InternalDamageRequest *request) {
  ++internal_calls;
  last_internal_request = *request;
  return 0;
}

BtechObjectId btech_context_stat_engine_dbref(const BtechContext *value) {
  (void)value;
  return 0;
}

int mech_position_z(const Mech *mech) {
  (void)mech;
  return 0;
}

void mech_flood_section(Mech *mech, int section, int elevation) {
  (void)mech;
  (void)section;
  (void)elevation;
  ++flood_count;
}

bool mech_section_is_destroyed(const Mech *mech, int section) {
  (void)mech;
  (void)section;
  return false;
}

int mech_location_maybe_breach(Mech *source, Mech *wounded, int hit_location) {
  (void)source;
  (void)wounded;
  (void)hit_location;
  ++breach_checks;
  return 0;
}

DbRef mech_carried_dbref(const Mech *mech) {
  (void)mech;
  return NOTHING;
}

const char *weapon_catalogue_name(int weapon_index) {
  (void)weapon_index;
  return "IS.Autocannon";
}

intptr_t damage_unused_stub(void);
intptr_t damage_unused_stub(void) { return 0; }

__asm__(
    ".globl bsuit_swarmer_find\n.set bsuit_swarmer_find, damage_unused_stub\n"
    ".globl bsuit_member_count\n.set bsuit_member_count, damage_unused_stub\n"
    ".globl mech_hit_location\n.set mech_hit_location, damage_unused_stub\n"
    ".globl mech_sprinting_set\n.set mech_sprinting_set, damage_unused_stub\n"
    ".globl mech_los_broadcast\n.set mech_los_broadcast, damage_unused_stub\n"
    ".globl mech_event_count\n.set mech_event_count, damage_unused_stub\n"
    ".globl mech_movemode_event\n.set mech_movemode_event, damage_unused_stub\n"
    ".globl mech_event_schedule\n.set mech_event_schedule, damage_unused_stub\n"
    ".globl mech_hidden_set\n.set mech_hidden_set, damage_unused_stub\n"
    ".globl mech_move_mode_locked\n.set mech_move_mode_locked, "
    "damage_unused_stub\n"
    ".globl mech_event_first_delay\n.set mech_event_first_delay, "
    "damage_unused_stub\n"
    ".globl mech_event_cancel\n.set mech_event_cancel, damage_unused_stub\n"
    ".globl btech_context_rotor_damage_divisor\n.set "
    "btech_context_rotor_damage_divisor, damage_unused_stub\n"
    ".globl gunnery_experience_award\n.set gunnery_experience_award, "
    "damage_unused_stub\n"
    ".globl mech_is_destroyed\n.set mech_is_destroyed, damage_unused_stub\n"
    ".globl btech_context_database\n.set btech_context_database, "
    "damage_unused_stub\n"
    ".globl mech_team\n.set mech_team, damage_unused_stub\n"
    ".globl piloting_experience_award\n.set piloting_experience_award, "
    "damage_unused_stub\n"
    ".globl mech_event_count\n.set mech_event_count, damage_unused_stub\n"
    ".globl mech_structural_integrity\n.set mech_structural_integrity, "
    "damage_unused_stub\n"
    ".globl mech_structural_integrity_set\n.set "
    "mech_structural_integrity_set, damage_unused_stub\n"
    ".globl mech_ood_damage\n.set mech_ood_damage, damage_unused_stub\n"
    ".globl personal_armor_reduce_damage\n.set "
    "personal_armor_reduce_damage, damage_unused_stub\n"
    ".globl btech_context_now\n.set btech_context_now, damage_unused_stub\n"
    ".globl mech_stagger_damage_append\n.set mech_stagger_damage_append, "
    "damage_unused_stub\n"
    ".globl headhitmwdamage\n.set headhitmwdamage, damage_unused_stub\n"
    ".globl mech_is_landed\n.set mech_is_landed, damage_unused_stub\n"
    ".globl mech_is_started\n.set mech_is_started, damage_unused_stub\n"
    ".globl mech_los_broadcast_unit\n.set mech_los_broadcast_unit, "
    "damage_unused_stub\n"
    ".globl mech_destroy\n.set mech_destroy, damage_unused_stub\n"
    ".globl mech_searchlight_destroy\n.set mech_searchlight_destroy, "
    "damage_unused_stub\n"
    ".globl btech_context_evaluation\n.set btech_context_evaluation, "
    "damage_unused_stub\n"
    ".globl mech_pilot_dbref\n.set mech_pilot_dbref, damage_unused_stub\n"
    ".globl mech_model_reference\n.set mech_model_reference, "
    "damage_unused_stub\n"
    ".globl tprintf\n.set tprintf, damage_unused_stub\n"
    ".globl notify_checked\n.set notify_checked, damage_unused_stub\n"
    ".globl mech_location_breach\n.set mech_location_breach, "
    "damage_unused_stub\n"
    ".globl mech_section_configuration_has\n.set "
    "mech_section_configuration_has, damage_unused_stub\n"
    ".globl mech_dropoff\n.set mech_dropoff, damage_unused_stub\n"
    ".globl btech_context_get_mech\n.set btech_context_get_mech, "
    "damage_unused_stub\n"
    ".globl mech_plasma_hit\n.set mech_plasma_hit, damage_unused_stub\n"
    ".globl mech_ammunition_dump_explode\n.set "
    "mech_ammunition_dump_explode, damage_unused_stub\n"
    ".globl game_object_has_flag\n.set game_object_has_flag, "
    "damage_unused_stub\n");

static void reset_fixture(void) {
  memset(&attacker, 0, sizeof(attacker));
  memset(&target, 0, sizeof(target));
  combat_map = nullptr;
  combat_safe = false;
  armor = 10;
  internal = 10;
  depleted_section = -1;
  fixture_class = CLASS_VEH_GROUND;
  transfer_location = 2;
  armor_calls = 0;
  internal_calls = 0;
  notification_count = 0;
  attacker_damage = 0;
  target_damage = 0;
  turn_damage = 0;
  flood_count = 0;
  breach_checks = 0;
  last_armor_request = (ArmorDamageRequest){0};
  last_internal_request = (InternalDamageRequest){0};
}

static MechDamageRequest request(void) {
  return (MechDamageRequest){
      .target = &target,
      .attacker = &attacker,
      .attack_pilot = 33,
      .hit_location = 1,
      .armor_damage = 5,
      .cause = 2,
      .base_to_hit = 7,
      .weapon_index = 4,
      .ammunition_mode = 8,
  };
}

static int test_armor_path(void) {
  reset_fixture();
  MechDamageRequest damage = request();
  mech_damage_apply(&damage);
  return armor_calls != 1 || internal_calls != 0 ||
         last_armor_request.section != 1 || last_armor_request.damage != 5 ||
         last_armor_request.weapon_index != 4 ||
         last_armor_request.ammunition_mode != 8 || attacker_damage != 5 ||
         target_damage != 5 || turn_damage != 5 || flood_count != 1 ||
         breach_checks != 1;
}

static int test_internal_path(void) {
  reset_fixture();
  MechDamageRequest damage = request();
  damage.armor_damage = 0;
  damage.internal_damage = 3;
  mech_damage_apply(&damage);
  return armor_calls != 0 || internal_calls != 1 ||
         last_internal_request.section != 1 ||
         last_internal_request.damage != 3;
}

static int test_forced_transfer(void) {
  reset_fixture();
  depleted_section = 1;
  fixture_class = CLASS_MECH;
  MechDamageRequest damage = request();
  damage.transfer = MECH_DAMAGE_FORCE_TRANSFER;
  mech_damage_apply(&damage);
  return armor_calls != 1 || last_armor_request.section != 2 ||
         attacker_damage != 0 || target_damage != 0;
}

static int test_combat_safe_short_circuit(void) {
  reset_fixture();
  combat_map = (BattleMap *)&target;
  combat_safe = true;
  MechDamageRequest damage = request();
  mech_damage_apply(&damage);
  return armor_calls != 0 || internal_calls != 0 || target_damage != 0 ||
         notification_count != 1;
}

int main(void) {
  int failures = 0;
  if (test_armor_path()) {
    fprintf(stderr, "armor path failed\n");
    ++failures;
  }
  if (test_internal_path()) {
    fprintf(stderr, "internal path failed\n");
    ++failures;
  }
  if (test_forced_transfer()) {
    fprintf(stderr, "forced transfer failed\n");
    ++failures;
  }
  if (test_combat_safe_short_circuit()) {
    fprintf(stderr, "combat-safe path failed\n");
    ++failures;
  }
  return failures != 0;
}
