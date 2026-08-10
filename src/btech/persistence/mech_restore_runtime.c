#include "ai_api.h"
#include "mech_persistence.h"
#include "mech_stagger.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "sqlite_internal.h"
#include <limits.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static int *runtime_restore_int_slot(int *values, size_t count, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, count, sizeof(*values), (size_t)index);
}

static char *runtime_restore_char_slot(char *values, size_t count, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, count, sizeof(*values), (size_t)index);
}

int btech_special_load_mech_runtime(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  DbRef mech_dbref;
  int result;
  int step;

  statement = NULL;
  result = sqlite3_prepare_v2(
               sqlite, "SELECT * FROM btech_mech_runtime ORDER BY mech_dbref;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0) {
      result = -1;
      break;
    }
    mech = btech_context_get_mech(context, mech_dbref);
    if (!mech) {
      result = -1;
      break;
    }
    mech_persistence_snapshot_export(mech, &snapshot);
    if (btech_special_column_char(statement, 1, &snapshot.runtime.jumptop) <
            0 ||
        btech_special_column_char(statement, 2, &snapshot.runtime.aim) < 0 ||
        btech_special_column_char(statement, 3, &snapshot.runtime.basetohit) <
            0 ||
        btech_special_column_char(statement, 4,
                                  &snapshot.runtime.pilotskillbase) < 0 ||
        btech_special_column_char(statement, 5, &snapshot.runtime.engineheat) <
            0 ||
        btech_special_column_char(statement, 6, &snapshot.runtime.masc_value) <
            0 ||
        btech_special_column_char(statement, 7, &snapshot.runtime.aim_type) <
            0 ||
        btech_special_column_char(statement, 8, &snapshot.runtime.sensor[0]) <
            0 ||
        btech_special_column_char(statement, 9, &snapshot.runtime.sensor[1]) <
            0 ||
        btech_special_column_uchar(statement, 10,
                                   &snapshot.runtime.fire_adjustment) < 0 ||
        btech_special_column_char(statement, 11, &snapshot.runtime.vis_mod) <
            0 ||
        btech_special_column_char(statement, 12,
                                  &snapshot.runtime.chargetimer) < 0 ||
        btech_special_column_real(statement, 13, &snapshot.runtime.chargedist) <
            0 ||
        btech_special_column_char(statement, 14,
                                  &snapshot.runtime.staggerstamp) < 0 ||
        btech_special_column_int(statement, 15, &snapshot.runtime.mech_prefs) <
            0 ||
        btech_special_column_short(statement, 16,
                                   &snapshot.runtime.jumplength) < 0 ||
        btech_special_column_short(statement, 17, &snapshot.runtime.goingx) <
            0 ||
        btech_special_column_short(statement, 18, &snapshot.runtime.goingy) <
            0 ||
        btech_special_column_short(statement, 19,
                                   &snapshot.runtime.desiredfacing) < 0 ||
        btech_special_column_short(statement, 20, &snapshot.runtime.angle) <
            0 ||
        btech_special_column_short(statement, 21,
                                   &snapshot.runtime.jumpheading) < 0 ||
        btech_special_column_short(statement, 22, &snapshot.runtime.targx) <
            0 ||
        btech_special_column_short(statement, 23, &snapshot.runtime.targy) <
            0 ||
        btech_special_column_short(statement, 24, &snapshot.runtime.targz) <
            0 ||
        btech_special_column_short(statement, 25,
                                   &snapshot.runtime.turretfacing) < 0 ||
        btech_special_column_short(statement, 26,
                                   &snapshot.runtime.turndamage) < 0 ||
        btech_special_column_short(statement, 27, &snapshot.runtime.lateral) <
            0 ||
        btech_special_column_short(statement, 28, &snapshot.runtime.num_seen) <
            0 ||
        btech_special_column_short(statement, 29, &snapshot.runtime.lx) < 0 ||
        btech_special_column_short(statement, 30, &snapshot.runtime.ly) < 0 ||
        btech_special_column_dbref(context->database, statement, 31,
                                   &snapshot.runtime.chgtarget) < 0 ||
        btech_special_column_dbref(context->database, statement, 32,
                                   &snapshot.runtime.dfatarget) < 0 ||
        btech_special_column_dbref(context->database, statement, 33,
                                   &snapshot.runtime.target) < 0 ||
        btech_special_column_dbref(context->database, statement, 34,
                                   &snapshot.runtime.swarming) < 0 ||
        btech_special_column_dbref(context->database, statement, 35,
                                   &snapshot.runtime.swarmedby) < 0 ||
        btech_special_column_dbref(context->database, statement, 36,
                                   &snapshot.runtime.carrying) < 0 ||
        btech_special_column_dbref(context->database, statement, 37,
                                   &snapshot.runtime.spotter) < 0 ||
        btech_special_column_real(statement, 38, &snapshot.runtime.heat) < 0 ||
        btech_special_column_real(statement, 39, &snapshot.runtime.weapheat) <
            0 ||
        btech_special_column_real(statement, 40, &snapshot.runtime.plus_heat) <
            0 ||
        btech_special_column_real(statement, 41, &snapshot.runtime.minus_heat) <
            0 ||
        btech_special_column_real(statement, 42, &snapshot.runtime.startfx) <
            0 ||
        btech_special_column_real(statement, 43, &snapshot.runtime.startfy) <
            0 ||
        btech_special_column_real(statement, 44, &snapshot.runtime.startfz) <
            0 ||
        btech_special_column_real(statement, 45, &snapshot.runtime.endfz) < 0 ||
        btech_special_column_real(statement, 46,
                                  &snapshot.runtime.verticalspeed) < 0 ||
        btech_special_column_real(statement, 47, &snapshot.runtime.speed) < 0 ||
        btech_special_column_real(statement, 48,
                                  &snapshot.runtime.desired_speed) < 0 ||
        btech_special_column_real(statement, 49, &snapshot.runtime.jumpspeed) <
            0 ||
        btech_special_column_int(statement, 50, &snapshot.runtime.critstatus) <
            0 ||
        btech_special_column_int(statement, 51, &snapshot.runtime.status) < 0 ||
        btech_special_column_int(statement, 52, &snapshot.runtime.status2) <
            0 ||
        btech_special_column_int(statement, 53, &snapshot.runtime.specials) <
            0 ||
        btech_special_column_int(statement, 54, &snapshot.runtime.specials2) <
            0 ||
        btech_special_column_int(statement, 55,
                                 &snapshot.runtime.specialsstatus) < 0 ||
        btech_special_column_int(statement, 56,
                                 &snapshot.runtime.tankcritstatus) < 0 ||
        btech_special_column_time(statement, 57,
                                  &snapshot.runtime.last_weapon_recycle) < 0 ||
        btech_special_column_int(statement, 58,
                                 &snapshot.runtime.cargo_weight) < 0 ||
        btech_special_column_int(statement, 59, &snapshot.runtime.lastrndu) <
            0 ||
        btech_special_column_int(statement, 60, &snapshot.runtime.rnd) < 0 ||
        btech_special_column_int(statement, 61, &snapshot.runtime.last_ds_msg) <
            0 ||
        btech_special_column_int(statement, 62, &snapshot.runtime.boom_start) <
            0 ||
        btech_special_column_int(statement, 63, &snapshot.runtime.maxfuel) <
            0 ||
        btech_special_column_int(statement, 64, &snapshot.runtime.lastused) <
            0 ||
        btech_special_column_int(statement, 65, &snapshot.runtime.cocoon) < 0 ||
        btech_special_column_int(statement, 66, &snapshot.runtime.commconv) <
            0 ||
        btech_special_column_int(statement, 67,
                                 &snapshot.runtime.commconv_last) < 0 ||
        btech_special_column_int(statement, 68, &snapshot.runtime.onumsinks) <
            0 ||
        btech_special_column_int(statement, 69, &snapshot.runtime.disabled_hs) <
            0 ||
        btech_special_column_long(statement, 70,
                                  &snapshot.runtime.autopilot_num) < 0 ||
        btech_special_column_int(statement, 71,
                                 &snapshot.runtime.heatboom_last) < 0 ||
        btech_special_column_time(statement, 72, &snapshot.runtime.sspin) < 0 ||
        btech_special_column_int(statement, 73, &snapshot.runtime.can_see) <
            0 ||
        btech_special_column_int(statement, 74, &snapshot.runtime.row) < 0 ||
        btech_special_column_int(statement, 75, &snapshot.runtime.rcw) < 0 ||
        btech_special_column_real(statement, 76, &snapshot.runtime.rspd) < 0 ||
        btech_special_column_int(statement, 77, &snapshot.runtime.erat) < 0 ||
        btech_special_column_int(statement, 78, &snapshot.runtime.per) < 0 ||
        btech_special_column_int(statement, 79, &snapshot.runtime.wxf) < 0 ||
        btech_special_column_int(statement, 80,
                                 &snapshot.runtime.last_startup) < 0 ||
        btech_special_column_int(statement, 81, &snapshot.runtime.maxsuits) <
            0 ||
        btech_special_column_int(statement, 82,
                                 &snapshot.runtime.infantry_specials) < 0 ||
        btech_special_column_char(statement, 83,
                                  &snapshot.runtime.scharge_value) < 0 ||
        btech_special_column_int(statement, 84,
                                 &snapshot.runtime.staggerDamage) < 0 ||
        btech_special_column_int(statement, 85,
                                 &snapshot.runtime.lastStaggerNotify) < 0 ||
        btech_special_column_int(statement, 86, &snapshot.runtime.critstatus2) <
            0 ||
        btech_special_column_real(statement, 87, &snapshot.runtime.xpmod) < 0 ||
        btech_special_column_int(statement, 88, &snapshot.runtime.shots_fired) <
            0 ||
        btech_special_column_int(statement, 89, &snapshot.runtime.shots_hit) <
            0 ||
        btech_special_column_int(statement, 90,
                                 &snapshot.runtime.shots_missed) < 0 ||
        btech_special_column_int(statement, 91,
                                 &snapshot.runtime.damage_taken) < 0 ||
        btech_special_column_int(statement, 92,
                                 &snapshot.runtime.damage_inflicted) < 0 ||
        btech_special_column_int(statement, 93,
                                 &snapshot.runtime.units_killed) < 0 ||
        btech_special_column_time(statement, 94,
                                  &snapshot.runtime.lastStaggerCheck) < 0)
      result = -1;
    else
      mech_persistence_runtime_restore(mech, &snapshot);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore reserved unit fields so a future release does not lose them. */
int btech_special_load_mech_unit_aux(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  DbRef current_mech;
  DbRef mech_dbref;
  int seen[11];
  int result;
  int slot;
  int step;
  int value;
#ifndef BT_CALCULATE_BV
  int index;
#endif

  statement = NULL;
  current_mech = NOTHING;
  mech = NULL;
  memset(seen, 0, sizeof(seen));
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT mech_dbref, slot, value FROM btech_mech_unit_aux "
               "ORDER BY mech_dbref, slot;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &slot) < 0 ||
        btech_special_column_int(statement, 2, &value) < 0 || slot < 0 ||
        slot >= 11) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech) {
#ifndef BT_CALCULATE_BV
        for (index = 0; index < 11; index++)
          if (!*runtime_restore_int_slot(seen, 11, index))
            result = -1;
#else
        if (!*runtime_restore_int_slot(seen, 11, 0) ||
            !*runtime_restore_int_slot(seen, 11, 8) ||
            !*runtime_restore_int_slot(seen, 11, 9) ||
            !*runtime_restore_int_slot(seen, 11, 10))
          result = -1;
#endif
        if (result < 0)
          break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      memset(seen, 0, sizeof(seen));
      mech_persistence_snapshot_export(mech, &snapshot);
    }
#ifdef BT_CALCULATE_BV
    if ((slot > 0 && slot < 8) || *runtime_restore_int_slot(seen, 11, slot)) {
#else
    if (*runtime_restore_int_slot(seen, 11, slot)) {
#endif
      result = -1;
      break;
    }
    *runtime_restore_int_slot(seen, 11, slot) = 1;
#ifndef BT_CALCULATE_BV
    if (slot < 8)
      snapshot.definition.unused[slot] = value;
    else if (value < CHAR_MIN || value > CHAR_MAX)
      result = -1;
    else
      *runtime_restore_char_slot(snapshot.definition.unused_char, 3, slot - 8) =
          (char)value;
#else
    if (slot == 0)
      snapshot.definition.mechbv_last = value;
    else if (value < CHAR_MIN || value > CHAR_MAX)
      result = -1;
    else
      *runtime_restore_char_slot(snapshot.definition.unused_char, 3, slot - 8) =
          (char)value;
#endif
    if (result == 0)
      mech_persistence_identity_restore(mech, &snapshot);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech) {
#ifndef BT_CALCULATE_BV
    for (index = 0; index < 11; index++)
      if (!*runtime_restore_int_slot(seen, 11, index))
        result = -1;
#else
    if (!*runtime_restore_int_slot(seen, 11, 0) ||
        !*runtime_restore_int_slot(seen, 11, 8) ||
        !*runtime_restore_int_slot(seen, 11, 9) ||
        !*runtime_restore_int_slot(seen, 11, 10))
      result = -1;
#endif
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore every reserved mech_rd integer in its fixed five-slot order. */
int btech_special_load_mech_runtime_unused(sqlite3 *sqlite,
                                           BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  DbRef current_mech;
  DbRef mech_dbref;
  int expected_slot;
  int result;
  int slot;
  int step;
  int value;

  statement = NULL;
  current_mech = NOTHING;
  expected_slot = 0;
  mech = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT mech_dbref, slot, value FROM btech_mech_runtime_unused "
               "ORDER BY mech_dbref, slot;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &slot) < 0 ||
        btech_special_column_int(statement, 2, &value) < 0) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_slot != 5) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_slot = 0;
      mech_persistence_snapshot_export(mech, &snapshot);
    }
    if (slot != expected_slot || slot >= 5) {
      result = -1;
      break;
    }
    *runtime_restore_int_slot(snapshot.runtime.unused, 5, slot) = value;
    mech_persistence_runtime_restore(mech, &snapshot);
    expected_slot++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && expected_slot != 5)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Rebuild stagger history in list order without loading the saved pointer. */
int btech_special_load_mech_stagger_damage(sqlite3 *sqlite,
                                           BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  DbRef current_mech;
  DbRef mech_dbref;
  DbRef attacker;
  time_t occurred_at;
  int amount;
  int counted;
  int expected_position;
  int position;
  int result;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  expected_position = 0;
  mech = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT mech_dbref, position, amount, occurred_at, "
               "attacker_dbref, counted "
               "FROM btech_mech_stagger_damage ORDER BY mech_dbref, position;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &position) < 0 ||
        btech_special_column_int(statement, 2, &amount) < 0 ||
        btech_special_column_time(statement, 3, &occurred_at) < 0 ||
        btech_special_column_dbref(context->database, statement, 4, &attacker) <
            0 ||
        btech_special_column_int(statement, 5, &counted) < 0 || counted < 0 ||
        counted > 1) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech || !mech_stagger_damage_history_is_empty(mech)) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_position = 0;
    }
    if (position != expected_position) {
      result = -1;
      break;
    }
    if (!mech_stagger_damage_append(
            &(StaggerDamageApplication){.mech = mech,
                                        .amount = amount,
                                        .occurred_at = occurred_at,
                                        .attacker = attacker,
                                        .counted = counted != 0})) {
      result = -1;
      break;
    }
    expected_position++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Resolve a preallocated special object and reject a row of the wrong type. */
