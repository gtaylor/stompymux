#include "ai_api.h"
#include "equipment_types.h"
#include "mech_persistence.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "sqlite_internal.h"
#include <limits.h>
#include <string.h>

int btech_special_load_mech_positions(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  DbRef mech_dbref;
  long pilot;
  float fx;
  float fy;
  float fz;
  float hexes_walked;
  int facing;
  int last_x;
  int last_y;
  int pilot_status;
  int result;
  int stall;
  int step;
  int team;
  int unusable_arcs;
  int x;
  int y;
  int z;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT mech_dbref, pilot_status, hexes_walked, facing, x, y, z, "
          "last_x, last_y, fx, fy, fz, team, unusable_arcs, stall, pilot "
          "FROM btech_mech_positions ORDER BY mech_dbref;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        !(mech = btech_context_get_mech(context, mech_dbref)) ||
        btech_special_column_int(statement, 1, &pilot_status) < 0 ||
        btech_special_column_real(statement, 2, &hexes_walked) < 0 ||
        btech_special_column_int(statement, 3, &facing) < 0 ||
        btech_special_column_int(statement, 4, &x) < 0 ||
        btech_special_column_int(statement, 5, &y) < 0 ||
        btech_special_column_int(statement, 6, &z) < 0 ||
        btech_special_column_int(statement, 7, &last_x) < 0 ||
        btech_special_column_int(statement, 8, &last_y) < 0 ||
        btech_special_column_real(statement, 9, &fx) < 0 ||
        btech_special_column_real(statement, 10, &fy) < 0 ||
        btech_special_column_real(statement, 11, &fz) < 0 ||
        btech_special_column_int(statement, 12, &team) < 0 ||
        btech_special_column_int(statement, 13, &unusable_arcs) < 0 ||
        btech_special_column_int(statement, 14, &stall) < 0 ||
        btech_special_column_long(statement, 15, &pilot) < 0 ||
        pilot_status < CHAR_MIN || pilot_status > CHAR_MAX ||
        facing < SHRT_MIN || facing > SHRT_MAX || x < SHRT_MIN ||
        x > SHRT_MAX || y < SHRT_MIN || y > SHRT_MAX || z < SHRT_MIN ||
        z > SHRT_MAX || last_x < SHRT_MIN || last_x > SHRT_MAX ||
        last_y < SHRT_MIN || last_y > SHRT_MAX ||
        (pilot != NOTHING && !is_good_obj(context->database, pilot))) {
      result = -1;
      break;
    }
    mech_persistence_snapshot_export(mech, &snapshot);
    snapshot.position.pilotstatus = (char)pilot_status;
    snapshot.position.hexes_walked = hexes_walked;
    snapshot.position.facing = (short)facing;
    snapshot.position.x = (short)x;
    snapshot.position.y = (short)y;
    snapshot.position.z = (short)z;
    snapshot.position.last_x = (short)last_x;
    snapshot.position.last_y = (short)last_y;
    snapshot.position.fx = fx;
    snapshot.position.fy = fy;
    snapshot.position.fz = fz;
    snapshot.position.team = team;
    snapshot.position.unusable_arcs = unusable_arcs;
    snapshot.position.stall = stall;
    snapshot.position.pilot = pilot;
    mech_persistence_position_restore(mech, &snapshot);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all bay dbref links in their fixed four-slot order. */
int btech_special_load_mech_bays(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  DbRef current_mech;
  DbRef mech_dbref;
  long bay_dbref;
  int bay_index;
  int expected_bay;
  int result;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  expected_bay = 0;
  mech = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT mech_dbref, bay_index, bay_dbref FROM btech_mech_bays "
               "ORDER BY mech_dbref, bay_index;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &bay_index) < 0 ||
        btech_special_column_long(statement, 2, &bay_dbref) < 0 ||
        (bay_dbref != NOTHING && !is_good_obj(context->database, bay_dbref))) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_bay != NUM_BAYS) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_bay = 0;
    }
    if (bay_index != expected_bay || bay_index >= NUM_BAYS) {
      result = -1;
      break;
    }
    mech_persistence_bay_restore(mech, bay_index, bay_dbref);
    expected_bay++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && expected_bay != NUM_BAYS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all independent turret dbref links in their fixed three-slot order.
 */
int btech_special_load_mech_turrets(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  DbRef current_mech;
  DbRef mech_dbref;
  long turret_dbref;
  int expected_turret;
  int result;
  int step;
  int turret_index;

  statement = NULL;
  current_mech = NOTHING;
  expected_turret = 0;
  mech = NULL;
  result = sqlite3_prepare_v2(sqlite,
                              "SELECT mech_dbref, turret_index, turret_dbref "
                              "FROM btech_mech_turrets "
                              "ORDER BY mech_dbref, turret_index;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &turret_index) < 0 ||
        btech_special_column_long(statement, 2, &turret_dbref) < 0 ||
        (turret_dbref != NOTHING &&
         !is_good_obj(context->database, turret_dbref))) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_turret != NUM_TURRETS) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_turret = 0;
    }
    if (turret_index != expected_turret || turret_index >= NUM_TURRETS) {
      result = -1;
      break;
    }
    mech_persistence_turret_restore(mech, turret_index, turret_dbref);
    expected_turret++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && expected_turret != NUM_TURRETS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore C3/C3i parent fields before their fixed indexed network rows. */
int btech_special_load_mech_c3(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  char channel_title[CHTITLELEN + 1];
  DbRef mech_dbref;
  long tag_target;
  long tagged_by;
  int c3_size;
  int c3i_size;
  int frequency_mode;
  int result;
  int step;
  int total_masters;
  int working_masters;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT mech_dbref, channel_title, c3i_size, c3_size, total_masters, "
          "working_masters, frequency_mode, tag_target, tagged_by "
          "FROM btech_mech_c3 ORDER BY mech_dbref;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        !(mech = btech_context_get_mech(context, mech_dbref)) ||
        btech_special_column_text(statement, 1, channel_title,
                                  sizeof(channel_title)) < 0 ||
        btech_special_column_int(statement, 2, &c3i_size) < 0 ||
        btech_special_column_int(statement, 3, &c3_size) < 0 ||
        btech_special_column_int(statement, 4, &total_masters) < 0 ||
        btech_special_column_int(statement, 5, &working_masters) < 0 ||
        btech_special_column_int(statement, 6, &frequency_mode) < 0 ||
        btech_special_column_long(statement, 7, &tag_target) < 0 ||
        btech_special_column_long(statement, 8, &tagged_by) < 0 ||
        c3i_size < -1 || c3i_size > C3I_NETWORK_SIZE || c3_size < -1 ||
        c3_size > C3_NETWORK_SIZE || total_masters < -1 ||
        total_masters > C3_NETWORK_SIZE || working_masters < -1 ||
        working_masters > C3_NETWORK_SIZE ||
        (tag_target != NOTHING &&
         !is_good_obj(context->database, tag_target)) ||
        (tagged_by != NOTHING && !is_good_obj(context->database, tagged_by))) {
      result = -1;
      break;
    }
    mech_persistence_snapshot_export(mech, &snapshot);
    memcpy(snapshot.network.C3ChanTitle, channel_title, sizeof(channel_title));
    snapshot.network.wC3iNetworkSize = c3i_size;
    snapshot.network.wC3NetworkSize = c3_size;
    snapshot.network.wTotalC3Masters = total_masters;
    snapshot.network.wWorkingC3Masters = working_masters;
    snapshot.network.C3FreqMode = frequency_mode;
    snapshot.network.tagTarget = tag_target;
    snapshot.network.taggedBy = tagged_by;
    mech_persistence_network_restore(mech, &snapshot);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore every C3i and C3 array element, including empty slots. */
int btech_special_load_mech_c3_nodes(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  DbRef current_mech;
  DbRef mech_dbref;
  long node_dbref;
  int expected_network;
  int expected_node;
  int network_type;
  int node_index;
  int result;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  expected_network = 0;
  expected_node = 0;
  mech = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT mech_dbref, network_type, node_index, node_dbref "
               "FROM btech_mech_c3_nodes ORDER BY mech_dbref, network_type, "
               "node_index;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &network_type) < 0 ||
        btech_special_column_int(statement, 2, &node_index) < 0 ||
        btech_special_column_long(statement, 3, &node_dbref) < 0 ||
        (node_dbref != NOTHING && node_dbref != 0 &&
         !btech_context_get_mech(context, node_dbref))) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && (expected_network != 2 || expected_node != 0)) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_network = 0;
      expected_node = 0;
    }
    if (network_type != expected_network || node_index != expected_node ||
        (network_type == 0 && node_index >= C3I_NETWORK_SIZE) ||
        (network_type == 1 && node_index >= C3_NETWORK_SIZE)) {
      result = -1;
      break;
    }
    mech_persistence_network_node_restore(mech, network_type, node_index,
                                          node_dbref);
    expected_node++;
    if ((expected_network == 0 && expected_node == C3I_NETWORK_SIZE) ||
        (expected_network == 1 && expected_node == C3_NETWORK_SIZE)) {
      expected_network++;
      expected_node = 0;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && (expected_network != 2 || expected_node != 0))
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore the complete NUM_TICS by TICLONGS bitmap matrix. */
int btech_special_load_mech_tics(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  DbRef current_mech;
  DbRef mech_dbref;
  unsigned long value;
  int expected_tic;
  int expected_word;
  int result;
  int step;
  int tic_index;
  int word_index;

  statement = NULL;
  current_mech = NOTHING;
  expected_tic = 0;
  expected_word = 0;
  mech = NULL;
  result = sqlite3_prepare_v2(sqlite,
                              "SELECT mech_dbref, tic_index, word_index, value "
                              "FROM btech_mech_tics "
                              "ORDER BY mech_dbref, tic_index, word_index;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &tic_index) < 0 ||
        btech_special_column_int(statement, 2, &word_index) < 0 ||
        btech_special_column_ulong(statement, 3, &value) < 0) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && (expected_tic != NUM_TICS || expected_word != 0)) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_tic = 0;
      expected_word = 0;
    }
    if (tic_index != expected_tic || word_index != expected_word ||
        tic_index >= NUM_TICS || word_index >= TICLONGS) {
      result = -1;
      break;
    }
    mech_persistence_tic_restore(mech, tic_index, word_index, value);
    if (++expected_word == TICLONGS) {
      expected_word = 0;
      expected_tic++;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && (expected_tic != NUM_TICS || expected_word != 0))
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all radio slots with their mode and fixed-size title buffer. */
int btech_special_load_mech_frequencies(sqlite3 *sqlite,
                                        BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  char title[CHTITLELEN + 1];
  DbRef current_mech;
  DbRef mech_dbref;
  int expected_frequency;
  int frequency;
  int frequency_index;
  int mode;
  int result;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  expected_frequency = 0;
  mech = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT mech_dbref, frequency_index, frequency, mode, title "
          "FROM btech_mech_frequencies ORDER BY mech_dbref, frequency_index;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &frequency_index) < 0 ||
        btech_special_column_int(statement, 2, &frequency) < 0 ||
        btech_special_column_int(statement, 3, &mode) < 0 ||
        btech_special_column_text(statement, 4, title, sizeof(title)) < 0) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_frequency != FREQS) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_frequency = 0;
    }
    if (frequency_index != expected_frequency || frequency_index >= FREQS) {
      result = -1;
      break;
    }
    mech_persistence_frequency_restore(mech, frequency_index, frequency, mode,
                                       title);
    expected_frequency++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && expected_frequency != FREQS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore the complete pointer-free mech_rd record from its named columns. */
