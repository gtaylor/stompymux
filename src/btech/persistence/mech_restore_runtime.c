#include "sqlite_internal.h"

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
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        !(mech = btech_context_get_mech(context, mech_dbref))) {
      result = -1;
      break;
    }
    mech_persistence_snapshot_export(mech, &snapshot);
#define RUNTIME_CHAR(column, field)                                            \
  btech_special_column_char(statement, column, &snapshot.runtime.field)
#define RUNTIME_SHORT(column, field)                                           \
  btech_special_column_short(statement, column, &snapshot.runtime.field)
#define RUNTIME_INT(column, field)                                             \
  btech_special_column_int(statement, column, &snapshot.runtime.field)
#define RUNTIME_REAL(column, field)                                            \
  btech_special_column_real(statement, column, &snapshot.runtime.field)
#define RUNTIME_DBREF(column, field)                                           \
  btech_special_column_dbref(context->database, statement, column,             \
                             &snapshot.runtime.field)
    if (RUNTIME_CHAR(1, jumptop) < 0 || RUNTIME_CHAR(2, aim) < 0 ||
        RUNTIME_CHAR(3, basetohit) < 0 || RUNTIME_CHAR(4, pilotskillbase) < 0 ||
        RUNTIME_CHAR(5, engineheat) < 0 || RUNTIME_CHAR(6, masc_value) < 0 ||
        RUNTIME_CHAR(7, aim_type) < 0 ||
        btech_special_column_char(statement, 8, &snapshot.runtime.sensor[0]) <
            0 ||
        btech_special_column_char(statement, 9, &snapshot.runtime.sensor[1]) <
            0 ||
        btech_special_column_uchar(statement, 10,
                                   &snapshot.runtime.fire_adjustment) < 0 ||
        RUNTIME_CHAR(11, vis_mod) < 0 || RUNTIME_CHAR(12, chargetimer) < 0 ||
        RUNTIME_REAL(13, chargedist) < 0 ||
        RUNTIME_CHAR(14, staggerstamp) < 0 || RUNTIME_INT(15, mech_prefs) < 0 ||
        RUNTIME_SHORT(16, jumplength) < 0 || RUNTIME_SHORT(17, goingx) < 0 ||
        RUNTIME_SHORT(18, goingy) < 0 || RUNTIME_SHORT(19, desiredfacing) < 0 ||
        RUNTIME_SHORT(20, angle) < 0 || RUNTIME_SHORT(21, jumpheading) < 0 ||
        RUNTIME_SHORT(22, targx) < 0 || RUNTIME_SHORT(23, targy) < 0 ||
        RUNTIME_SHORT(24, targz) < 0 || RUNTIME_SHORT(25, turretfacing) < 0 ||
        RUNTIME_SHORT(26, turndamage) < 0 || RUNTIME_SHORT(27, lateral) < 0 ||
        RUNTIME_SHORT(28, num_seen) < 0 || RUNTIME_SHORT(29, lx) < 0 ||
        RUNTIME_SHORT(30, ly) < 0 || RUNTIME_DBREF(31, chgtarget) < 0 ||
        RUNTIME_DBREF(32, dfatarget) < 0 || RUNTIME_DBREF(33, target) < 0 ||
        RUNTIME_DBREF(34, swarming) < 0 || RUNTIME_DBREF(35, swarmedby) < 0 ||
        RUNTIME_DBREF(36, carrying) < 0 || RUNTIME_DBREF(37, spotter) < 0 ||
        RUNTIME_REAL(38, heat) < 0 || RUNTIME_REAL(39, weapheat) < 0 ||
        RUNTIME_REAL(40, plus_heat) < 0 || RUNTIME_REAL(41, minus_heat) < 0 ||
        RUNTIME_REAL(42, startfx) < 0 || RUNTIME_REAL(43, startfy) < 0 ||
        RUNTIME_REAL(44, startfz) < 0 || RUNTIME_REAL(45, endfz) < 0 ||
        RUNTIME_REAL(46, verticalspeed) < 0 || RUNTIME_REAL(47, speed) < 0 ||
        RUNTIME_REAL(48, desired_speed) < 0 ||
        RUNTIME_REAL(49, jumpspeed) < 0 || RUNTIME_INT(50, critstatus) < 0 ||
        RUNTIME_INT(51, status) < 0 || RUNTIME_INT(52, status2) < 0 ||
        RUNTIME_INT(53, specials) < 0 || RUNTIME_INT(54, specials2) < 0 ||
        RUNTIME_INT(55, specialsstatus) < 0 ||
        RUNTIME_INT(56, tankcritstatus) < 0 ||
        btech_special_column_time(statement, 57,
                                  &snapshot.runtime.last_weapon_recycle) < 0 ||
        RUNTIME_INT(58, cargo_weight) < 0 || RUNTIME_INT(59, lastrndu) < 0 ||
        RUNTIME_INT(60, rnd) < 0 || RUNTIME_INT(61, last_ds_msg) < 0 ||
        RUNTIME_INT(62, boom_start) < 0 || RUNTIME_INT(63, maxfuel) < 0 ||
        RUNTIME_INT(64, lastused) < 0 || RUNTIME_INT(65, cocoon) < 0 ||
        RUNTIME_INT(66, commconv) < 0 || RUNTIME_INT(67, commconv_last) < 0 ||
        RUNTIME_INT(68, onumsinks) < 0 || RUNTIME_INT(69, disabled_hs) < 0 ||
        RUNTIME_INT(70, autopilot_num) < 0 ||
        RUNTIME_INT(71, heatboom_last) < 0 || RUNTIME_INT(72, sspin) < 0 ||
        RUNTIME_INT(73, can_see) < 0 || RUNTIME_INT(74, row) < 0 ||
        RUNTIME_INT(75, rcw) < 0 || RUNTIME_REAL(76, rspd) < 0 ||
        RUNTIME_INT(77, erat) < 0 || RUNTIME_INT(78, per) < 0 ||
        RUNTIME_INT(79, wxf) < 0 || RUNTIME_INT(80, last_startup) < 0 ||
        RUNTIME_INT(81, maxsuits) < 0 ||
        RUNTIME_INT(82, infantry_specials) < 0 ||
        RUNTIME_CHAR(83, scharge_value) < 0 ||
        RUNTIME_INT(84, staggerDamage) < 0 ||
        RUNTIME_INT(85, lastStaggerNotify) < 0 ||
        RUNTIME_INT(86, critstatus2) < 0 || RUNTIME_REAL(87, xpmod) < 0 ||
        RUNTIME_INT(88, shots_fired) < 0 || RUNTIME_INT(89, shots_hit) < 0 ||
        RUNTIME_INT(90, shots_missed) < 0 ||
        RUNTIME_INT(91, damage_taken) < 0 ||
        RUNTIME_INT(92, damage_inflicted) < 0 ||
        RUNTIME_INT(93, units_killed) < 0 ||
        btech_special_column_time(statement, 94,
                                  &snapshot.runtime.lastStaggerCheck) < 0)
      result = -1;
    else
      mech_persistence_runtime_restore(mech, &snapshot);
#undef RUNTIME_CHAR
#undef RUNTIME_SHORT
#undef RUNTIME_INT
#undef RUNTIME_REAL
#undef RUNTIME_DBREF
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
          if (!seen[index])
            result = -1;
#else
        if (!seen[0] || !seen[8] || !seen[9] || !seen[10])
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
    if ((slot > 0 && slot < 8) || seen[slot]) {
#else
    if (seen[slot]) {
#endif
      result = -1;
      break;
    }
    seen[slot] = 1;
#ifndef BT_CALCULATE_BV
    if (slot < 8)
      snapshot.definition.unused[slot] = value;
    else if (value < CHAR_MIN || value > CHAR_MAX)
      result = -1;
    else
      snapshot.definition.unused_char[slot - 8] = (char)value;
#else
    if (slot == 0)
      snapshot.definition.mechbv_last = value;
    else if (value < CHAR_MIN || value > CHAR_MAX)
      result = -1;
    else
      snapshot.definition.unused_char[slot - 8] = (char)value;
#endif
    if (result == 0)
      mech_persistence_identity_restore(mech, &snapshot);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech) {
#ifndef BT_CALCULATE_BV
    for (index = 0; index < 11; index++)
      if (!seen[index])
        result = -1;
#else
    if (!seen[0] || !seen[8] || !seen[9] || !seen[10])
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
    snapshot.runtime.unused[slot] = value;
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
    if (!mech_stagger_damage_append(mech, amount, occurred_at, attacker,
                                    counted != 0)) {
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
