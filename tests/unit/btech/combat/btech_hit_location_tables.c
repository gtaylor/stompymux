#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>

#include "aero_move_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "crit_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_hitloc_api.h"
#include "mech_hitloc_internal.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"

static Mech fixture_mech;
static BtechContext *const fixture_context = (BtechContext *)&fixture_mech;
static UnitClass fixture_class = CLASS_MECH;
static int fixture_roll = 2;
static bool fixture_turret_internal;
static bool fixture_crittable;
static int rotor_destroyed_count;
static int rotor_damaged_count;
static int main_weapon_destroyed_count;

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return fixture_context;
}

long btech_random_range(BtechContext *context [[maybe_unused]], long low,
                        long high [[maybe_unused]]) {
  return low;
}

int btech_random_range_int(BtechContext *context [[maybe_unused]], int low,
                           int high [[maybe_unused]]) {
  return low;
}

int btech_random_roll(BtechContext *context [[maybe_unused]]) {
  return fixture_roll;
}

int btech_context_critical_level(const BtechContext *context [[maybe_unused]]) {
  return 40;
}

void btech_context_hit_roll_record(BtechContext *context [[maybe_unused]],
                                   int roll [[maybe_unused]]) {}

bool btech_context_uses_advanced_vehicle_criticals(const BtechContext *context
                                                   [[maybe_unused]]) {
  return false;
}

bool btech_context_uses_advanced_vtol_criticals(const BtechContext *context
                                                [[maybe_unused]]) {
  return false;
}

bool btech_context_uses_exile_stun_code(const BtechContext *context
                                        [[maybe_unused]]) {
  return false;
}

bool btech_context_uses_fasa_criticals(const BtechContext *context
                                       [[maybe_unused]]) {
  return false;
}

UnitClass mech_class(const Mech *mech [[maybe_unused]]) {
  return fixture_class;
}

MechConditionSummary mech_condition_summary(const Mech *mech [[maybe_unused]]) {
  return (MechConditionSummary){0};
}

int mech_technology_flags(const Mech *mech [[maybe_unused]]) { return 0; }

int mech_section_internal(const Mech *mech [[maybe_unused]], int section) {
  return fixture_turret_internal && section == TURRET;
}

int mech_section_original_internal(const Mech *mech [[maybe_unused]],
                                   int section [[maybe_unused]]) {
  return 0;
}

bool mech_section_is_crittable(Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]],
                               CriticalThreshold threshold [[maybe_unused]]) {
  return fixture_crittable;
}

void mech_motive_system_hit(Mech *mech [[maybe_unused]],
                            int modifier [[maybe_unused]]) {}

int mech_battle_suit_hit_location(Mech *mech [[maybe_unused]]) { return -1; }

int mech_head_hit_modify(int hit_group [[maybe_unused]],
                         Mech *mech [[maybe_unused]]) {
  return HEAD;
}

int mech_spheroid_rear_section(const Mech *mech [[maybe_unused]], int section) {
  return section;
}

DbRef mech_dbref(const Mech *mech [[maybe_unused]]) { return 1; }

void btech_channel_send(BtechContext *context [[maybe_unused]],
                        BtechChannel channel [[maybe_unused]],
                        const char *format [[maybe_unused]], ...) {}

void dropship_bridge_hit(Mech *mech [[maybe_unused]]) {}

void mech_heat_sink_destroy(Mech *mech [[maybe_unused]],
                            int hitloc [[maybe_unused]]) {}

void mech_weapon_destroy_random(Mech *mech [[maybe_unused]],
                                int hitloc [[maybe_unused]]) {}

void mech_vtol_rotor_destroyed_critical_apply(Mech *obj_mech [[maybe_unused]],
                                              Mech *obj_attacker
                                              [[maybe_unused]],
                                              int los [[maybe_unused]]) {
  ++rotor_destroyed_count;
}

void mech_vtol_rotor_damaged_critical_apply(Mech *obj_mech [[maybe_unused]]) {
  ++rotor_damaged_count;
}

void mech_main_weapon_destroy(Mech *mech [[maybe_unused]]) {
  ++main_weapon_destroyed_count;
}

HitLocationResult mech_fasa_hit_location(Mech *mech [[maybe_unused]],
                                         int hit_group [[maybe_unused]],
                                         HitLocationResult result) {
  return result;
}

static int standard_hit_location(int direction) {
  bool iscritical = false;
  bool isrear = false;
  return mech_hit_location(&fixture_mech, direction, &iscritical, &isrear);
}

static int critproof_hit_location(int direction) {
  bool iscritical = false;
  return mech_critproof_hit_location(&fixture_mech, direction, &iscritical);
}

static int advanced_hit_location(int direction) {
  HitLocationResult result = mech_advanced_vehicle_hit_location(
      &fixture_mech, direction, (HitLocationResult){0});
  return result.location;
}

static int fasa_mech_table_location(int direction) {
  HitLocationResult result = fasa_mech_hit_location(
      &fixture_mech, direction, (HitLocationResult){0}, fixture_roll);
  return result.location;
}

static int check_standard_table(const int expected[11], int direction,
                                const char *name) {
  for (int index = 0; index < 11; ++index) {
    fixture_roll = index + 2;
    const int expected_location = *(const int *)checked_storage_at_const(
        expected, 11, sizeof(*expected), (size_t)index);
    if (standard_hit_location(direction) != expected_location) {
      fprintf(stderr, "%s roll %d returned the wrong location\n", name,
              fixture_roll);
      return 1;
    }
  }
  return 0;
}

static int check_critproof_table(const int expected[11], int direction,
                                 const char *name) {
  for (int index = 0; index < 11; ++index) {
    fixture_roll = index + 2;
    const int expected_location = *(const int *)checked_storage_at_const(
        expected, 11, sizeof(*expected), (size_t)index);
    if (critproof_hit_location(direction) != expected_location) {
      fprintf(stderr, "%s roll %d returned the wrong location\n", name,
              fixture_roll);
      return 1;
    }
  }
  return 0;
}

static int check_advanced_table(const int expected[11], int direction,
                                const char *name) {
  for (int index = 0; index < 11; ++index) {
    fixture_roll = index + 2;
    const int expected_location = *(const int *)checked_storage_at_const(
        expected, 11, sizeof(*expected), (size_t)index);
    if (advanced_hit_location(direction) != expected_location) {
      fprintf(stderr, "%s roll %d returned the wrong location\n", name,
              fixture_roll);
      return 1;
    }
  }
  return 0;
}

static int test_advanced_tables(void) {
  /* Current advanced vehicle tables from the published record-sheet tables. */
  static const int GROUND_LEFT[11] = {
      LSIDE, LSIDE, LSIDE, FSIDE, LSIDE, LSIDE,
      LSIDE, BSIDE, LSIDE, LSIDE, LSIDE,
  };
  static const int GROUND_RIGHT[11] = {
      RSIDE, RSIDE, RSIDE, FSIDE, RSIDE, RSIDE,
      RSIDE, BSIDE, RSIDE, RSIDE, RSIDE,
  };
  static const int GROUND_FRONT[11] = {
      FSIDE, FSIDE, FSIDE, RSIDE, FSIDE, FSIDE,
      FSIDE, LSIDE, LSIDE, LSIDE, LSIDE,
  };
  static const int GROUND_REAR[11] = {
      BSIDE, BSIDE, BSIDE, LSIDE, BSIDE, BSIDE,
      BSIDE, RSIDE, RSIDE, RSIDE, RSIDE,
  };

  fixture_crittable = false;
  fixture_turret_internal = false;
  fixture_class = CLASS_VEH_GROUND;
  if (check_advanced_table(GROUND_LEFT, LEFTSIDE, "advanced left vehicle") ||
      check_advanced_table(GROUND_RIGHT, RIGHTSIDE, "advanced right vehicle") ||
      check_advanced_table(GROUND_FRONT, FRONT, "advanced front vehicle") ||
      check_advanced_table(GROUND_REAR, BACK, "advanced rear vehicle"))
    return 1;

  fixture_class = CLASS_VTOL;
  static const int VTOL_LEFT[11] = {
      LSIDE, LSIDE, ROTOR, FSIDE, LSIDE, LSIDE,
      LSIDE, BSIDE, ROTOR, ROTOR, ROTOR,
  };
  static const int VTOL_RIGHT[11] = {
      RSIDE, RSIDE, ROTOR, FSIDE, RSIDE, RSIDE,
      RSIDE, BSIDE, ROTOR, ROTOR, ROTOR,
  };
  static const int VTOL_FRONT[11] = {
      FSIDE, FSIDE, ROTOR, RSIDE, FSIDE, FSIDE,
      FSIDE, LSIDE, ROTOR, ROTOR, ROTOR,
  };
  static const int VTOL_REAR[11] = {
      BSIDE, BSIDE, ROTOR, LSIDE, BSIDE, BSIDE,
      BSIDE, RSIDE, ROTOR, ROTOR, ROTOR,
  };
  if (check_advanced_table(VTOL_LEFT, LEFTSIDE, "advanced left VTOL") ||
      check_advanced_table(VTOL_RIGHT, RIGHTSIDE, "advanced right VTOL") ||
      check_advanced_table(VTOL_FRONT, FRONT, "advanced front VTOL") ||
      check_advanced_table(VTOL_REAR, BACK, "advanced rear VTOL"))
    return 1;

  fixture_class = CLASS_VTOL;
  fixture_turret_internal = true;
  fixture_crittable = true;
  fixture_roll = 4;
  HitLocationResult turret_result = mech_advanced_vehicle_hit_location(
      &fixture_mech, FRONT, (HitLocationResult){0});
  if (turret_result.location != TURRET || turret_result.critical)
    return 1;

  fixture_roll = 8;
  HitLocationResult side_critical_result = mech_advanced_vehicle_hit_location(
      &fixture_mech, LEFTSIDE, (HitLocationResult){0});
  if (side_critical_result.location != LSIDE || !side_critical_result.critical)
    return 1;

  fixture_class = CLASS_VEH_GROUND;
  fixture_roll = 12;
  HitLocationResult result = mech_advanced_vehicle_hit_location(
      &fixture_mech, FRONT, (HitLocationResult){0});
  return result.location != TURRET || !result.critical;
}

static int test_mech_tables(void) {
  /* BattleTech Master Rules' published BattleMech hit-location table. */
  static const int LEFT[11] = {LTORSO, LLEG,   LARM, LARM, LLEG, LTORSO,
                               CTORSO, RTORSO, RARM, RLEG, HEAD};
  static const int FRONT_TABLE[11] = {CTORSO, RARM, RARM, RLEG, RTORSO, CTORSO,
                                      LTORSO, LLEG, LARM, LARM, HEAD};
  static const int RIGHT[11] = {RTORSO, RLEG,   RARM, RARM, RLEG, RTORSO,
                                CTORSO, LTORSO, LARM, LLEG, HEAD};

  fixture_class = CLASS_MECH;
  fixture_turret_internal = false;
  if (check_standard_table(LEFT, LEFTSIDE, "standard left mech") ||
      check_standard_table(FRONT_TABLE, FRONT, "standard front mech") ||
      check_standard_table(FRONT_TABLE, BACK, "standard rear mech") ||
      check_standard_table(RIGHT, RIGHTSIDE, "standard right mech"))
    return 1;

  if (check_critproof_table(LEFT, LEFTSIDE, "crit-proof left mech") ||
      check_critproof_table(FRONT_TABLE, FRONT, "crit-proof front mech") ||
      check_critproof_table(FRONT_TABLE, BACK, "crit-proof rear mech") ||
      check_critproof_table(RIGHT, RIGHTSIDE, "crit-proof right mech"))
    return 1;

  for (int direction_index = 0; direction_index < 4; ++direction_index) {
    const int direction = direction_index == 0   ? LEFTSIDE
                          : direction_index == 1 ? RIGHTSIDE
                          : direction_index == 2 ? FRONT
                                                 : BACK;
    const int *table = direction_index == 0   ? LEFT
                       : direction_index == 1 ? RIGHT
                       : direction_index == 2 ? FRONT_TABLE
                                              : FRONT_TABLE;
    for (int index = 0; index < 11; ++index) {
      fixture_roll = index + 2;
      const int expected = *(const int *)checked_storage_at_const(
          table, 11, sizeof(*table), (size_t)index);
      if (fasa_mech_table_location(direction) != expected)
        return 1;
    }
  }

  fixture_crittable = true;
  fixture_roll = 2;
  bool iscritical = false;
  bool isrear = false;
  if (mech_hit_location(&fixture_mech, LEFTSIDE, &iscritical, &isrear) !=
          LTORSO ||
      !iscritical)
    return 1;
  return 0;
}

static int test_ground_and_naval_tables(void) {
  /* The standard and crit-proof vehicle paths retain the legacy table: rolls
   * 3-9 all hit the attacker's side. The modern opposite-side results belong
   * to the advanced table below, so keep this as an explicit behavior test
   * until the rules mode is chosen for the legacy path. */
  static const int LEFT[11] = {LSIDE, LSIDE, LSIDE, LSIDE, LSIDE, LSIDE,
                               LSIDE, LSIDE, LSIDE, LSIDE, LSIDE};
  static const int RIGHT[11] = {RSIDE, RSIDE, RSIDE, RSIDE, RSIDE, RSIDE,
                                RSIDE, RSIDE, RSIDE, RSIDE, RSIDE};
  static const int FRONT_TABLE[11] = {FSIDE, FSIDE, FSIDE, FSIDE, FSIDE, FSIDE,
                                      FSIDE, FSIDE, FSIDE, FSIDE, FSIDE};
  static const int REAR_TABLE[11] = {BSIDE, BSIDE, BSIDE, BSIDE, BSIDE, BSIDE,
                                     BSIDE, BSIDE, BSIDE, BSIDE, BSIDE};

  fixture_crittable = false;
  fixture_turret_internal = false;
  fixture_class = CLASS_VEH_GROUND;
  if (check_standard_table(LEFT, LEFTSIDE, "standard left vehicle") ||
      check_standard_table(FRONT_TABLE, FRONT, "standard front vehicle") ||
      check_standard_table(REAR_TABLE, BACK, "standard rear vehicle") ||
      check_standard_table(RIGHT, RIGHTSIDE, "standard right vehicle") ||
      check_critproof_table(LEFT, LEFTSIDE, "crit-proof left vehicle") ||
      check_critproof_table(FRONT_TABLE, FRONT, "crit-proof front vehicle") ||
      check_critproof_table(REAR_TABLE, BACK, "crit-proof rear vehicle") ||
      check_critproof_table(RIGHT, RIGHTSIDE, "crit-proof right vehicle"))
    return 1;

  fixture_class = CLASS_VEH_NAVAL;
  if (check_standard_table(LEFT, LEFTSIDE, "standard left naval vehicle") ||
      check_standard_table(FRONT_TABLE, FRONT,
                           "standard front naval vehicle") ||
      check_standard_table(REAR_TABLE, BACK, "standard rear naval vehicle") ||
      check_standard_table(RIGHT, RIGHTSIDE, "standard right naval vehicle") ||
      check_critproof_table(LEFT, LEFTSIDE, "crit-proof left naval vehicle") ||
      check_critproof_table(FRONT_TABLE, FRONT,
                            "crit-proof front naval vehicle") ||
      check_critproof_table(REAR_TABLE, BACK,
                            "crit-proof rear naval vehicle") ||
      check_critproof_table(RIGHT, RIGHTSIDE, "crit-proof right naval vehicle"))
    return 1;

  fixture_roll = 12;
  bool left_critical = false;
  bool right_critical = false;
  bool isrear = false;
  (void)mech_hit_location(&fixture_mech, LEFTSIDE, &left_critical, &isrear);
  (void)mech_hit_location(&fixture_mech, RIGHTSIDE, &right_critical, &isrear);
  if (left_critical || right_critical)
    return 1;
  fixture_crittable = true;
  left_critical = false;
  right_critical = false;
  (void)mech_hit_location(&fixture_mech, LEFTSIDE, &left_critical, &isrear);
  (void)mech_hit_location(&fixture_mech, RIGHTSIDE, &right_critical, &isrear);
  if (!left_critical || !right_critical)
    return 1;
  fixture_crittable = false;

  /* The standard and crit-proof paths retain the pre-advanced VTOL table:
   * rolls 2-4 and 10-12 hit the rotor, while 5-9 hit the attacker's side.
   * This is intentionally documented as current behavior; the modern
   * advanced table is tested separately below. */
  static const int VTOL_LEFT[11] = {ROTOR, ROTOR, ROTOR, LSIDE, LSIDE, LSIDE,
                                    LSIDE, LSIDE, ROTOR, ROTOR, ROTOR};
  static const int VTOL_RIGHT[11] = {ROTOR, ROTOR, ROTOR, RSIDE, RSIDE, RSIDE,
                                     RSIDE, RSIDE, ROTOR, ROTOR, ROTOR};
  static const int VTOL_FRONT[11] = {ROTOR, ROTOR, ROTOR, FSIDE, FSIDE, FSIDE,
                                     FSIDE, FSIDE, ROTOR, ROTOR, ROTOR};
  static const int VTOL_REAR[11] = {ROTOR, ROTOR, ROTOR, BSIDE, BSIDE, BSIDE,
                                    BSIDE, BSIDE, ROTOR, ROTOR, ROTOR};
  fixture_class = CLASS_VTOL;
  if (check_standard_table(VTOL_LEFT, LEFTSIDE, "standard left VTOL") ||
      check_standard_table(VTOL_RIGHT, RIGHTSIDE, "standard right VTOL") ||
      check_standard_table(VTOL_FRONT, FRONT, "standard front VTOL") ||
      check_standard_table(VTOL_REAR, BACK, "standard rear VTOL") ||
      check_critproof_table(VTOL_LEFT, LEFTSIDE, "crit-proof left VTOL") ||
      check_critproof_table(VTOL_RIGHT, RIGHTSIDE, "crit-proof right VTOL") ||
      check_critproof_table(VTOL_FRONT, FRONT, "crit-proof front VTOL") ||
      check_critproof_table(VTOL_REAR, BACK, "crit-proof rear VTOL"))
    return 1;

  static const int TURRETED_FRONT[11] = {FSIDE,  FSIDE,  FSIDE, FSIDE,
                                         FSIDE,  FSIDE,  FSIDE, FSIDE,
                                         TURRET, TURRET, TURRET};
  static const int TURRETED_REAR[11] = {BSIDE,  BSIDE,  BSIDE, BSIDE,
                                        BSIDE,  BSIDE,  BSIDE, BSIDE,
                                        TURRET, TURRET, TURRET};
  static const int TURRETED_LEFT[11] = {LSIDE,  LSIDE,  LSIDE, LSIDE,
                                        LSIDE,  LSIDE,  LSIDE, LSIDE,
                                        TURRET, TURRET, LSIDE};
  static const int TURRETED_RIGHT[11] = {RSIDE,  RSIDE,  RSIDE, RSIDE,
                                         RSIDE,  RSIDE,  RSIDE, RSIDE,
                                         TURRET, TURRET, RSIDE};

  fixture_class = CLASS_VEH_GROUND;
  fixture_turret_internal = true;
  if (check_standard_table(TURRETED_LEFT, LEFTSIDE,
                           "standard turreted left vehicle") ||
      check_standard_table(TURRETED_RIGHT, RIGHTSIDE,
                           "standard turreted right vehicle") ||
      check_standard_table(TURRETED_FRONT, FRONT,
                           "standard turreted front vehicle") ||
      check_standard_table(TURRETED_REAR, BACK,
                           "standard turreted rear vehicle") ||
      check_critproof_table(TURRETED_FRONT, FRONT,
                            "crit-proof turreted front vehicle") ||
      check_critproof_table(TURRETED_REAR, BACK,
                            "crit-proof turreted rear vehicle"))
    return 1;

  fixture_class = CLASS_VEH_NAVAL;
  if (check_standard_table(TURRETED_FRONT, FRONT,
                           "standard turreted front naval vehicle"))
    return 1;
  return 0;
}

static int test_aerospace_tables(void) {
  static const int AERO_FRONT[11] = {
      AERO_NOSE, AERO_NOSE,  AERO_RWING, AERO_RWING, AERO_NOSE, AERO_NOSE,
      AERO_NOSE, AERO_LWING, AERO_RWING, AERO_NOSE,  AERO_NOSE,
  };
  static const int AERO_LEFT[11] = {
      AERO_AFT,   AERO_LWING, AERO_AFT, AERO_NOSE,  AERO_LWING, AERO_LWING,
      AERO_LWING, AERO_NOSE,  AERO_AFT, AERO_LWING, AERO_AFT,
  };
  static const int AERO_RIGHT[11] = {
      AERO_AFT,   AERO_RWING, AERO_AFT, AERO_NOSE,  AERO_RWING, AERO_RWING,
      AERO_RWING, AERO_NOSE,  AERO_AFT, AERO_RWING, AERO_AFT,
  };
  static const int AERO_REAR[11] = {
      AERO_AFT, AERO_RWING, AERO_RWING, AERO_RWING, AERO_AFT, AERO_RWING,
      AERO_AFT, AERO_LWING, AERO_RWING, AERO_RWING, AERO_AFT,
  };

  fixture_class = CLASS_AERO;
  fixture_crittable = false;
  fixture_turret_internal = false;
  if (check_standard_table(AERO_FRONT, FRONT, "standard front aerospace") ||
      check_standard_table(AERO_LEFT, LEFTSIDE, "standard left aerospace") ||
      check_standard_table(AERO_RIGHT, RIGHTSIDE, "standard right aerospace") ||
      check_standard_table(AERO_REAR, BACK, "standard rear aerospace"))
    return 1;

  static const int DS_FRONT[11] = {
      DS_NOSE, DS_NOSE,  DS_LWING, DS_RWING, DS_NOSE, DS_NOSE,
      DS_NOSE, DS_LWING, DS_LWING, DS_NOSE,  DS_NOSE,
  };
  static const int DS_LEFT[11] = {
      DS_NOSE,  DS_LWING, DS_LWING, DS_LWING, DS_LWING, DS_LWING,
      DS_LWING, DS_NOSE,  DS_LWING, DS_LWING, DS_LWING,
  };
  static const int DS_RIGHT[11] = {
      DS_NOSE,  DS_RWING, DS_RWING, DS_RWING, DS_RWING, DS_RWING,
      DS_RWING, DS_NOSE,  DS_RWING, DS_RWING, DS_RWING,
  };
  static const int DS_REAR[11] = {
      DS_AFT, DS_AFT,   DS_AFT, DS_RWING, DS_AFT, DS_AFT,
      DS_AFT, DS_LWING, DS_AFT, DS_AFT,   DS_AFT,
  };

  fixture_class = CLASS_DS;
  if (check_standard_table(DS_FRONT, FRONT, "standard front dropship") ||
      check_standard_table(DS_LEFT, LEFTSIDE, "standard left dropship") ||
      check_standard_table(DS_RIGHT, RIGHTSIDE, "standard right dropship") ||
      check_standard_table(DS_REAR, BACK, "standard rear dropship"))
    return 1;

  fixture_class = CLASS_SPHEROID_DS;
  return check_standard_table(DS_FRONT, FRONT,
                              "standard front spheroid dropship") ||
         check_standard_table(DS_LEFT, LEFTSIDE,
                              "standard left spheroid dropship") ||
         check_standard_table(DS_RIGHT, RIGHTSIDE,
                              "standard right spheroid dropship") ||
         check_standard_table(DS_REAR, BACK, "standard rear spheroid dropship");
}

static int test_fasa_tables(void) {
  /* FASA-era VTOL and naval tables; these are kept separate from the modern
   * advanced tables because the published rule sets differ. */
  static const int LEFT[11] = {ROTOR, ROTOR, ROTOR, ROTOR, LSIDE, LSIDE,
                               LSIDE, 0,     ROTOR, ROTOR, ROTOR};
  static const int RIGHT[11] = {ROTOR, ROTOR, ROTOR, ROTOR, RSIDE, RSIDE,
                                RSIDE, 0,     ROTOR, ROTOR, ROTOR};
  static const int FRONT_TABLE[11] = {ROTOR, ROTOR, ROTOR, ROTOR, FSIDE, FSIDE,
                                      FSIDE, FSIDE, ROTOR, ROTOR, ROTOR};
  static const int REAR_TABLE[11] = {ROTOR, ROTOR, ROTOR, ROTOR, BSIDE, BSIDE,
                                     BSIDE, BSIDE, ROTOR, ROTOR, ROTOR};
  fixture_class = CLASS_VTOL;
  fixture_turret_internal = false;
  for (int direction_index = 0; direction_index < 4; ++direction_index) {
    const int direction = direction_index == 0   ? LEFTSIDE
                          : direction_index == 1 ? RIGHTSIDE
                          : direction_index == 2 ? FRONT
                                                 : BACK;
    const int *table = direction_index == 0   ? LEFT
                       : direction_index == 1 ? RIGHT
                       : direction_index == 2 ? FRONT_TABLE
                                              : REAR_TABLE;
    for (int index = 0; index < 11; ++index) {
      fixture_roll = index + 2;
      rotor_destroyed_count = 0;
      rotor_damaged_count = 0;
      main_weapon_destroyed_count = 0;
      HitLocationResult result = fasa_vtol_naval_hit_location(
          &fixture_mech, direction, (HitLocationResult){0}, fixture_roll);
      const int expected_location = *(const int *)checked_storage_at_const(
          table, 11, sizeof(*table), (size_t)index);
      if (result.location != expected_location) {
        fprintf(stderr, "FASA VTOL direction %d roll %d returned %d\n",
                direction, fixture_roll, result.location);
        return 1;
      }
      const bool rotor_critical = fixture_roll == 2 || fixture_roll == 12;
      if (result.critical != rotor_critical)
        return 1;
      const bool destroys_rotor = fixture_roll == 2 || fixture_roll == 3;
      const bool damages_rotor = fixture_roll == 4 || fixture_roll == 5 ||
                                 fixture_roll == 10 || fixture_roll == 11 ||
                                 fixture_roll == 12;
      const bool destroys_main_weapon =
          fixture_roll == 9 && direction_index < 2;
      if ((rotor_destroyed_count != (destroys_rotor ? 1 : 0)) ||
          (rotor_damaged_count != (damages_rotor ? 1 : 0)) ||
          (main_weapon_destroyed_count != (destroys_main_weapon ? 1 : 0)))
        return 1;
    }
  }

  fixture_class = CLASS_VEH_NAVAL;
  fixture_turret_internal = false;
  for (int direction_index = 0; direction_index < 4; ++direction_index) {
    const int direction = direction_index == 0   ? LEFTSIDE
                          : direction_index == 1 ? RIGHTSIDE
                          : direction_index == 2 ? FRONT
                                                 : BACK;
    const int side = direction_index == 0   ? LSIDE
                     : direction_index == 1 ? RSIDE
                     : direction_index == 2 ? FSIDE
                                            : BSIDE;
    for (int index = 0; index < 11; ++index) {
      fixture_roll = index + 2;
      HitLocationResult result = fasa_vtol_naval_hit_location(
          &fixture_mech, direction, (HitLocationResult){0}, fixture_roll);
      if (result.location != side)
        return 1;
    }
  }
  fixture_roll = 12;
  fixture_crittable = false;
  if (fasa_vtol_naval_hit_location(&fixture_mech, LEFTSIDE,
                                   (HitLocationResult){0}, fixture_roll)
          .critical ||
      fasa_vtol_naval_hit_location(&fixture_mech, RIGHTSIDE,
                                   (HitLocationResult){0}, fixture_roll)
          .critical)
    return 1;
  fixture_crittable = true;
  if (!fasa_vtol_naval_hit_location(&fixture_mech, LEFTSIDE,
                                    (HitLocationResult){0}, fixture_roll)
           .critical ||
      !fasa_vtol_naval_hit_location(&fixture_mech, RIGHTSIDE,
                                    (HitLocationResult){0}, fixture_roll)
           .critical)
    return 1;
  fixture_crittable = false;
  fixture_turret_internal = true;
  static const int TURRETED_LEFT[11] = {
      LSIDE, LSIDE, LSIDE,  LSIDE,  LSIDE, LSIDE,
      LSIDE, LSIDE, TURRET, TURRET, LSIDE,
  };
  static const int TURRETED_FRONT[11] = {
      FSIDE, FSIDE, FSIDE,  FSIDE,  FSIDE,  FSIDE,
      FSIDE, FSIDE, TURRET, TURRET, TURRET,
  };
  for (int index = 0; index < 11; ++index) {
    fixture_roll = index + 2;
    if (fasa_vtol_naval_hit_location(&fixture_mech, LEFTSIDE,
                                     (HitLocationResult){0}, fixture_roll)
            .location !=
        *(const int *)checked_storage_at_const(
            TURRETED_LEFT, 11, sizeof(*TURRETED_LEFT), (size_t)index))
      return 1;
    if (fasa_vtol_naval_hit_location(&fixture_mech, FRONT,
                                     (HitLocationResult){0}, fixture_roll)
            .location !=
        *(const int *)checked_storage_at_const(
            TURRETED_FRONT, 11, sizeof(*TURRETED_FRONT), (size_t)index))
      return 1;
  }
  return 0;
}

int main(void) {
  int failures = 0;
  if (test_mech_tables()) {
    fprintf(stderr, "mech hit-location tables failed\n");
    ++failures;
  }
  if (test_ground_and_naval_tables()) {
    fprintf(stderr, "ground/naval hit-location tables failed\n");
    ++failures;
  }
  if (test_advanced_tables()) {
    fprintf(stderr, "advanced hit-location tables failed\n");
    ++failures;
  }
  if (test_aerospace_tables()) {
    fprintf(stderr, "aerospace hit-location tables failed\n");
    ++failures;
  }
  if (test_fasa_tables()) {
    fprintf(stderr, "FASA hit-location tables failed\n");
    ++failures;
  }
  return failures != 0;
}
