#include "btech/context.h"
#include "btech/persistence/schema.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/utf8.h"
#include <float.h>
#include <limits.h>
#include <math.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#define BTECH_PERSISTENCE_PREPARE_IMPLEMENTATION
#include "sqlite_internal.h"

/* Explicit map and repair-event tables are the first BTech SQLite mirror. */
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Woverlength-strings"
const char BTECH_SPECIAL_SCHEMA_SQL[] =
    "CREATE TABLE btech_persistence_metadata ("
    " id INTEGER PRIMARY KEY CHECK (id = 1),"
    " schema_name TEXT NOT NULL CHECK (schema_name = 'stompymux-btech'),"
    " schema_version INTEGER NOT NULL CHECK (schema_version = 3)"
    ");"
    "CREATE TABLE btech_maps ("
    " dbref INTEGER PRIMARY KEY, map_name TEXT NOT NULL, width INTEGER NOT "
    "NULL,"
    " height INTEGER NOT NULL, temperature INTEGER NOT NULL, gravity INTEGER "
    "NOT NULL,"
    " cloudbase INTEGER NOT NULL, visibility INTEGER NOT NULL, max_visibility "
    "INTEGER NOT NULL,"
    " light INTEGER NOT NULL, wind_direction INTEGER NOT NULL, wind_speed "
    "INTEGER NOT NULL,"
    " reserved INTEGER NOT NULL, flags INTEGER NOT NULL, cf INTEGER NOT NULL, "
    "cf_max INTEGER NOT NULL,"
    " on_map INTEGER NOT NULL, build_flag INTEGER NOT NULL, first_free INTEGER "
    "NOT NULL,"
    " moves INTEGER NOT NULL, move_mod INTEGER NOT NULL, sensor_flags INTEGER "
    "NOT NULL,"
    " regen_factor INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_map_hexes ("
    " map_dbref INTEGER NOT NULL, x INTEGER NOT NULL, y INTEGER NOT NULL, "
    "value INTEGER NOT NULL,"
    " PRIMARY KEY (map_dbref, x, y)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_map_slots ("
    " map_dbref INTEGER NOT NULL, slot INTEGER NOT NULL, mech_dbref INTEGER "
    "NOT NULL,"
    " mech_flags INTEGER NOT NULL, PRIMARY KEY (map_dbref, slot)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_map_los ("
    " map_dbref INTEGER NOT NULL, source_slot INTEGER NOT NULL, target_slot "
    "INTEGER NOT NULL,"
    " flags INTEGER NOT NULL, PRIMARY KEY (map_dbref, source_slot, target_slot)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_map_objects ("
    " map_dbref INTEGER NOT NULL, object_type INTEGER NOT NULL, ordinal "
    "INTEGER NOT NULL,"
    " x INTEGER NOT NULL, y INTEGER NOT NULL, object_dbref INTEGER NOT NULL,"
    " data_char INTEGER NOT NULL, data_short INTEGER NOT NULL, data_int "
    "INTEGER NOT NULL,"
    " PRIMARY KEY (map_dbref, object_type, ordinal)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_map_bits ("
    " map_dbref INTEGER NOT NULL, y INTEGER NOT NULL, byte_index INTEGER NOT "
    "NULL,"
    " value INTEGER NOT NULL, PRIMARY KEY (map_dbref, y, byte_index)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_repair_events ("
    " event_id INTEGER PRIMARY KEY, mech_dbref INTEGER NOT NULL, event_type "
    "INTEGER NOT NULL,"
    " remaining_ticks INTEGER NOT NULL, event_data INTEGER NOT NULL, is_fake "
    "INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_mechrep ("
    " dbref INTEGER PRIMARY KEY, current_target INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_turrets ("
    " dbref INTEGER PRIMARY KEY, arcs INTEGER NOT NULL, parent INTEGER NOT "
    "NULL,"
    " gunner INTEGER NOT NULL, target INTEGER NOT NULL, target_x INTEGER NOT "
    "NULL,"
    " target_y INTEGER NOT NULL, target_z INTEGER NOT NULL, lock_mode INTEGER "
    "NOT NULL"
    ");"
    "CREATE TABLE btech_turret_tics ("
    " turret_dbref INTEGER NOT NULL, tic_index INTEGER NOT NULL, value INTEGER "
    "NOT NULL,"
    " PRIMARY KEY (turret_dbref, tic_index)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_autopilots ("
    " dbref INTEGER PRIMARY KEY, mech_dbref INTEGER NOT NULL, map_dbref "
    "INTEGER NOT NULL,"
    " speed_percent INTEGER NOT NULL, offset_x INTEGER NOT NULL, offset_y "
    "INTEGER NOT NULL,"
    " verbose_level INTEGER NOT NULL, target INTEGER NOT NULL, target_score "
    "INTEGER NOT NULL,"
    " target_threshold INTEGER NOT NULL, target_update_tick INTEGER NOT NULL,"
    " chase_target INTEGER NOT NULL, chase_update_tick INTEGER NOT NULL,"
    " follow_update_tick INTEGER NOT NULL, flags INTEGER NOT NULL, "
    "mech_max_range INTEGER NOT NULL,"
    " roam_type INTEGER NOT NULL, roam_update_tick INTEGER NOT NULL, "
    "roam_target_x INTEGER NOT NULL,"
    " roam_target_y INTEGER NOT NULL, roam_anchor_x INTEGER NOT NULL, "
    "roam_anchor_y INTEGER NOT NULL,"
    " roam_anchor_distance INTEGER NOT NULL, ahead_ok INTEGER NOT NULL, "
    "auto_cmode INTEGER NOT NULL,"
    " auto_cdist INTEGER NOT NULL, auto_goweight INTEGER NOT NULL, "
    "auto_fweight INTEGER NOT NULL,"
    " auto_nervous INTEGER NOT NULL, b_msc INTEGER NOT NULL, w_msc INTEGER NOT "
    "NULL, b_bsc INTEGER NOT NULL,"
    " w_bsc INTEGER NOT NULL, b_dan INTEGER NOT NULL, w_dan INTEGER NOT NULL, "
    "last_upd INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_autopilot_commands ("
    " autopilot_dbref INTEGER NOT NULL, position INTEGER NOT NULL, "
    "command_enum INTEGER NOT NULL, arg_count INTEGER NOT NULL,"
    " PRIMARY KEY (autopilot_dbref, position)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_autopilot_command_args ("
    " autopilot_dbref INTEGER NOT NULL, command_position INTEGER NOT NULL, "
    "argument_index INTEGER NOT NULL, value TEXT NOT NULL,"
    " PRIMARY KEY (autopilot_dbref, command_position, argument_index)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_autopilot_path ("
    " autopilot_dbref INTEGER NOT NULL, position INTEGER NOT NULL, x INTEGER "
    "NOT NULL, y INTEGER NOT NULL,"
    " parent_x INTEGER NOT NULL, parent_y INTEGER NOT NULL, g_score INTEGER "
    "NOT NULL, h_score INTEGER NOT NULL,"
    " f_score INTEGER NOT NULL, hex_offset INTEGER NOT NULL, PRIMARY KEY "
    "(autopilot_dbref, position)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mechs ("
    " dbref INTEGER PRIMARY KEY, id_0 INTEGER NOT NULL, id_1 INTEGER NOT NULL, "
    "brief INTEGER NOT NULL,"
    " map_number INTEGER NOT NULL, map_dbref INTEGER NOT NULL, mech_name TEXT "
    "NOT NULL, mech_type TEXT NOT NULL,"
    " unit_era TEXT NOT NULL, unit_tro TEXT NOT NULL, unit_class INTEGER NOT "
    "NULL, movement_type INTEGER NOT NULL,"
    " tactical_range INTEGER NOT NULL, lrs_range INTEGER NOT NULL, scan_range "
    "INTEGER NOT NULL, heat_sinks INTEGER NOT NULL,"
    " heat_sink_override INTEGER NOT NULL, computer INTEGER NOT NULL, radio "
    "INTEGER NOT NULL, radio_info INTEGER NOT NULL,"
    " structural_integrity INTEGER NOT NULL, structural_integrity_original "
    "INTEGER NOT NULL, radio_range INTEGER NOT NULL,"
    " fuel INTEGER NOT NULL, fuel_original INTEGER NOT NULL, tons INTEGER NOT "
    "NULL, walk_speed INTEGER NOT NULL,"
    " run_speed INTEGER NOT NULL, max_speed REAL NOT NULL, template_max_speed "
    "REAL NOT NULL, battle_value INTEGER NOT NULL,"
    " cargo_space INTEGER NOT NULL, targeting_computer INTEGER NOT NULL, "
    "carrier_max_tons INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_mech_sections ("
    " mech_dbref INTEGER NOT NULL, section INTEGER NOT NULL, armor INTEGER NOT "
    "NULL, internal INTEGER NOT NULL, rear INTEGER NOT NULL,"
    " armor_original INTEGER NOT NULL, internal_original INTEGER NOT NULL, "
    "rear_original INTEGER NOT NULL, base_to_hit INTEGER NOT NULL,"
    " config INTEGER NOT NULL, recycle INTEGER NOT NULL, specials INTEGER NOT "
    "NULL, PRIMARY KEY (mech_dbref, section)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_criticals ("
    " mech_dbref INTEGER NOT NULL, section INTEGER NOT NULL, slot INTEGER NOT "
    "NULL, brand INTEGER NOT NULL, data INTEGER NOT NULL,"
    " item_type INTEGER NOT NULL, fire_mode INTEGER NOT NULL, ammo_mode "
    "INTEGER NOT NULL, damage_flags INTEGER NOT NULL,"
    " desired_ammo_location INTEGER NOT NULL, PRIMARY KEY (mech_dbref, "
    "section, slot)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_positions ("
    " mech_dbref INTEGER PRIMARY KEY, pilot_status INTEGER NOT NULL,"
    " hexes_walked REAL NOT NULL, facing INTEGER NOT NULL, x INTEGER NOT NULL, "
    "y INTEGER NOT NULL, z INTEGER NOT NULL,"
    " last_x INTEGER NOT NULL, last_y INTEGER NOT NULL, fx REAL NOT NULL, fy "
    "REAL NOT NULL, fz REAL NOT NULL,"
    " team INTEGER NOT NULL, unusable_arcs INTEGER NOT NULL, stall INTEGER NOT "
    "NULL, pilot INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_mech_bays ("
    " mech_dbref INTEGER NOT NULL, bay_index INTEGER NOT NULL, bay_dbref "
    "INTEGER NOT NULL,"
    " PRIMARY KEY (mech_dbref, bay_index)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_turrets ("
    " mech_dbref INTEGER NOT NULL, turret_index INTEGER NOT NULL, turret_dbref "
    "INTEGER NOT NULL,"
    " PRIMARY KEY (mech_dbref, turret_index)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_c3 ("
    " mech_dbref INTEGER PRIMARY KEY, channel_title TEXT NOT NULL, c3i_size "
    "INTEGER NOT NULL, c3_size INTEGER NOT NULL,"
    " total_masters INTEGER NOT NULL, working_masters INTEGER NOT NULL, "
    "frequency_mode INTEGER NOT NULL,"
    " tag_target INTEGER NOT NULL, tagged_by INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_mech_c3_nodes ("
    " mech_dbref INTEGER NOT NULL, network_type INTEGER NOT NULL, node_index "
    "INTEGER NOT NULL, node_dbref INTEGER NOT NULL,"
    " PRIMARY KEY (mech_dbref, network_type, node_index)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_tics ("
    " mech_dbref INTEGER NOT NULL, tic_index INTEGER NOT NULL, word_index "
    "INTEGER NOT NULL, value INTEGER NOT NULL,"
    " PRIMARY KEY (mech_dbref, tic_index, word_index)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_frequencies ("
    " mech_dbref INTEGER NOT NULL, frequency_index INTEGER NOT NULL, frequency "
    "INTEGER NOT NULL, mode INTEGER NOT NULL,"
    " title TEXT NOT NULL, PRIMARY KEY (mech_dbref, frequency_index)"
    ") WITHOUT ROWID;"
    /* time_t fields are Unix wall-clock timestamps stored as signed INTEGERs.
     */
    "CREATE TABLE btech_mech_runtime ("
    " mech_dbref INTEGER PRIMARY KEY, jumptop INTEGER NOT NULL, aim INTEGER "
    "NOT NULL, basetohit INTEGER NOT NULL,"
    " pilotskillbase INTEGER NOT NULL, engineheat INTEGER NOT NULL, masc_value "
    "INTEGER NOT NULL, aim_type INTEGER NOT NULL,"
    " sensor_primary INTEGER NOT NULL, sensor_secondary INTEGER NOT NULL, "
    "fire_adjustment INTEGER NOT NULL, vis_mod INTEGER NOT NULL,"
    " charge_timer INTEGER NOT NULL, charge_distance REAL NOT NULL, "
    "stagger_stamp INTEGER NOT NULL, mech_prefs INTEGER NOT NULL,"
    " jump_length INTEGER NOT NULL, going_x INTEGER NOT NULL, going_y INTEGER "
    "NOT NULL, desired_facing INTEGER NOT NULL,"
    " angle INTEGER NOT NULL, jump_heading INTEGER NOT NULL, target_x INTEGER "
    "NOT NULL, target_y INTEGER NOT NULL,"
    " target_z INTEGER NOT NULL, turret_facing INTEGER NOT NULL, turn_damage "
    "INTEGER NOT NULL, lateral INTEGER NOT NULL,"
    " num_seen INTEGER NOT NULL, lx INTEGER NOT NULL, ly INTEGER NOT NULL, "
    "charge_target INTEGER NOT NULL,"
    " dfa_target INTEGER NOT NULL, target INTEGER NOT NULL, swarming INTEGER "
    "NOT NULL, swarmed_by INTEGER NOT NULL,"
    " carrying INTEGER NOT NULL, spotter INTEGER NOT NULL, heat REAL NOT NULL, "
    "weapon_heat REAL NOT NULL,"
    " plus_heat REAL NOT NULL, minus_heat REAL NOT NULL, start_fx REAL NOT "
    "NULL, start_fy REAL NOT NULL,"
    " start_fz REAL NOT NULL, end_fz REAL NOT NULL, vertical_speed REAL NOT "
    "NULL, speed REAL NOT NULL,"
    " desired_speed REAL NOT NULL, jump_speed REAL NOT NULL, crit_status "
    "INTEGER NOT NULL, status INTEGER NOT NULL,"
    " status2 INTEGER NOT NULL, specials INTEGER NOT NULL, specials2 INTEGER "
    "NOT NULL, specials_status INTEGER NOT NULL,"
    " tank_crit_status INTEGER NOT NULL, last_weapon_recycle INTEGER NOT NULL, "
    "cargo_weight INTEGER NOT NULL,"
    " last_random_update INTEGER NOT NULL, random_seed INTEGER NOT NULL, "
    "last_ds_message INTEGER NOT NULL,"
    " boom_start INTEGER NOT NULL, max_fuel INTEGER NOT NULL, last_used "
    "INTEGER NOT NULL, cocoon INTEGER NOT NULL,"
    " commconv INTEGER NOT NULL, commconv_last INTEGER NOT NULL, "
    "original_heat_sinks INTEGER NOT NULL,"
    " disabled_heat_sinks INTEGER NOT NULL, autopilot_num INTEGER NOT NULL, "
    "heatboom_last INTEGER NOT NULL,"
    " spin_start INTEGER NOT NULL, can_see INTEGER NOT NULL, row_weight "
    "INTEGER NOT NULL, carried_weight INTEGER NOT NULL,"
    " relative_speed REAL NOT NULL, era_tick INTEGER NOT NULL, per INTEGER NOT "
    "NULL, wxf INTEGER NOT NULL,"
    " last_startup INTEGER NOT NULL, max_suits INTEGER NOT NULL, "
    "infantry_specials INTEGER NOT NULL,"
    " supercharger_value INTEGER NOT NULL, stagger_damage INTEGER NOT NULL, "
    "last_stagger_notify INTEGER NOT NULL,"
    " crit_status2 INTEGER NOT NULL, xp_modifier REAL NOT NULL, shots_fired "
    "INTEGER NOT NULL, shots_hit INTEGER NOT NULL,"
    " shots_missed INTEGER NOT NULL, damage_taken INTEGER NOT NULL, "
    "damage_inflicted INTEGER NOT NULL,"
    " units_killed INTEGER NOT NULL, last_stagger_check INTEGER NOT NULL"
    ");"
    "CREATE TABLE btech_mech_runtime_unused ("
    " mech_dbref INTEGER NOT NULL, slot INTEGER NOT NULL, value INTEGER NOT "
    "NULL,"
    " PRIMARY KEY (mech_dbref, slot)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_unit_aux ("
    " mech_dbref INTEGER NOT NULL, slot INTEGER NOT NULL, value INTEGER NOT "
    "NULL,"
    " PRIMARY KEY (mech_dbref, slot)"
    ") WITHOUT ROWID;"
    "CREATE TABLE btech_mech_stagger_damage ("
    " mech_dbref INTEGER NOT NULL, position INTEGER NOT NULL, amount INTEGER "
    "NOT NULL, occurred_at INTEGER NOT NULL,"
    " attacker_dbref INTEGER NOT NULL, counted INTEGER NOT NULL, PRIMARY KEY "
    "(mech_dbref, position)"
    ") WITHOUT ROWID;";
#pragma GCC diagnostic pop

/*
 * CTest can force one named BTech writer statement to fail. This code is
 * absent from production builds, and is scoped to an in-progress SQLite
 * extension write so reads and unrelated SQLite users are unaffected.
 */
#ifdef BTMUX_PERSISTENCE_TESTING
static const char *btech_special_test_fault_table;
static const char *btech_special_test_fault_phase;
static int btech_special_test_fault_active;
static int btech_special_test_fault_triggered;

void btech_special_test_reset_fault(void) {
  btech_special_test_fault_table = getenv("BTMUX_TEST_BTECH_FAIL_TABLE");
  btech_special_test_fault_phase = getenv("BTMUX_TEST_BTECH_FAIL_PHASE");
  btech_special_test_fault_active =
      btech_special_test_fault_table && btech_special_test_fault_table[0] &&
      btech_special_test_fault_phase && btech_special_test_fault_phase[0];
  btech_special_test_fault_triggered = 0;
}

static int btech_special_test_should_fail(const char *sql, const char *phase) {
  if (!btech_special_test_fault_active || btech_special_test_fault_triggered ||
      !sql || strcmp(phase, btech_special_test_fault_phase) ||
      !strstr(sql, btech_special_test_fault_table))
    return 0;
  btech_special_test_fault_triggered = 1;
  return 1;
}
#else
void btech_special_test_reset_fault(void) {}

static int btech_special_test_should_fail(const char *sql, const char *phase) {
  (void)sql;
  (void)phase;
  return 0;
}
#endif

/* Interpose only in this translation unit so prepare failures are testable. */
int btech_special_prepare_v2(sqlite3 *sqlite, const char *sql, int byte_count,
                             sqlite3_stmt **statement, const char **tail) {
  if (btech_special_test_should_fail(sql, "prepare")) {
    *statement = NULL;
    return SQLITE_ERROR;
  }
  return sqlite3_prepare_v2(sqlite, sql, byte_count, statement, tail);
}

#define SQLITE3_PREPARE_V2 btech_special_prepare_v2

int btech_special_exec(sqlite3 *sqlite, const char *sql) {
  char *error = NULL;
  int rc = sqlite3_exec(sqlite, sql, NULL, NULL, &error);

  sqlite3_free(error);
  return rc == SQLITE_OK ? 0 : -1;
}

int btech_special_step(sqlite3_stmt *statement) {
  if (btech_special_test_should_fail(sqlite3_sql(statement), "step") ||
      sqlite3_step(statement) != SQLITE_DONE ||
      sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

int btech_special_bind_int(sqlite3_stmt *statement, int index,
                           sqlite3_int64 value) {
  return sqlite3_bind_int64(statement, index, value) == SQLITE_OK ? 0 : -1;
}

int btech_special_bind_real(sqlite3_stmt *statement, int index, double value) {
  return sqlite3_bind_double(statement, index, value) == SQLITE_OK ? 0 : -1;
}

/* Mark the schema version in every snapshot, including snapshots with no BTech
 * objects. */
int btech_special_store_metadata(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  int result;

  statement = NULL;
  result = SQLITE3_PREPARE_V2(sqlite,
                              "INSERT INTO btech_persistence_metadata "
                              "(id, schema_name, schema_version) "
                              "VALUES (1, 'stompymux-btech', ?);",
                              -1, &statement, NULL) == SQLITE_OK &&
                   btech_special_bind_int(
                       statement, 1, BTECH_PERSISTENCE_SCHEMA_VERSION) == 0 &&
                   btech_special_step(statement) == 0
               ? 0
               : -1;
  sqlite3_finalize(statement);
  return result;
}

/* Read an SQLite INTEGER only when it fits the destination C integer. */
int btech_special_column_int(sqlite3_stmt *statement, int column, int *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < INT_MIN || number > INT_MAX)
    return -1;
  *value = (int)number;
  return 0;
}

/* Read an SQLite INTEGER only when it fits MUX's dbref/long representation. */
int btech_special_column_long(sqlite3_stmt *statement, int column,
                              long *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < LONG_MIN || number > LONG_MAX)
    return -1;
  *value = (long)number;
  return 0;
}

/* Read a non-negative SQLite INTEGER only when it fits an unsigned int. */
int btech_special_column_uint(sqlite3_stmt *statement, int column,
                              unsigned int *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < 0 || (sqlite3_uint64)number > UINT_MAX)
    return -1;
  *value = (unsigned int)number;
  return 0;
}

/* Read a non-negative SQLite INTEGER only when it fits an unsigned long. */
int btech_special_column_ulong(sqlite3_stmt *statement, int column,
                               unsigned long *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < 0 || (sqlite3_uint64)number > ULONG_MAX)
    return -1;
  *value = (unsigned long)number;
  return 0;
}

int btech_special_column_char(sqlite3_stmt *statement, int column,
                              char *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 ||
      number < CHAR_MIN || number > CHAR_MAX)
    return -1;
  *value = (char)number;
  return 0;
}

int btech_special_column_uchar(sqlite3_stmt *statement, int column,
                               unsigned char *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 || number < 0 ||
      number > UCHAR_MAX)
    return -1;
  *value = (unsigned char)number;
  return 0;
}

int btech_special_column_short(sqlite3_stmt *statement, int column,
                               short *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 ||
      number < SHRT_MIN || number > SHRT_MAX)
    return -1;
  *value = (short)number;
  return 0;
}

int btech_special_column_ushort(sqlite3_stmt *statement, int column,
                                unsigned short *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 || number < 0 ||
      number > USHRT_MAX)
    return -1;
  *value = (unsigned short)number;
  return 0;
}

int btech_special_column_dbref(GameDatabase *database, sqlite3_stmt *statement,
                               int column, DbRef *value) {
  if (btech_special_column_long(statement, column, value) < 0 ||
      (*value != NOTHING && !is_good_obj(database, *value)))
    return -1;
  return 0;
}

int btech_special_column_time(sqlite3_stmt *statement, int column,
                              time_t *value) {
  sqlite3_int64 number;
  time_t converted;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  converted = (time_t)number;
  if ((sqlite3_int64)converted != number)
    return -1;
  *value = converted;
  return 0;
}

/* SQLite accepts INTEGER values for real-valued fields but never NaN or Inf. */
int btech_special_column_real(sqlite3_stmt *statement, int column,
                              float *value) {
  double number;
  int type;

  type = sqlite3_column_type(statement, column);
  if (type != SQLITE_FLOAT && type != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_double(statement, column);
  if (!isfinite(number) || number < -(double)FLT_MAX ||
      number > (double)FLT_MAX)
    return -1;
  *value = (float)number;
  return 0;
}

/* Copy NUL-free SQLite text only when it fits the fixed BTech destination. */
int btech_special_column_text(sqlite3_stmt *statement, int column,
                              char *destination, size_t destination_size) {
  const unsigned char *text;
  int length;

  if (!destination_size ||
      sqlite3_column_type(statement, column) != SQLITE_TEXT)
    return -1;
  text = sqlite3_column_text(statement, column);
  length = sqlite3_column_bytes(statement, column);
  if (!text || length < 0 || (size_t)length >= destination_size ||
      (int)strlen((const char *)text) != length ||
      !utf8_validate((const char *)text, (size_t)length))
    return -1;
  memcpy(destination, text, (size_t)length + 1);
  return 0;
}

/* Require exactly one schema metadata row written by this persistence layer. */
int btech_special_validate_metadata(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  int matching_rows;
  int result;

  statement = NULL;
  result =
      SQLITE3_PREPARE_V2(sqlite,
                         "SELECT count(*) FROM btech_persistence_metadata "
                         "WHERE id = 1 AND schema_name = 'stompymux-btech' "
                         "AND schema_version = 3;",
                         -1, &statement, NULL) == SQLITE_OK &&
              sqlite3_step(statement) == SQLITE_ROW &&
              btech_special_column_int(statement, 0, &matching_rows) == 0 &&
              matching_rows == 1 && sqlite3_step(statement) == SQLITE_DONE
          ? 0
          : -1;
  sqlite3_finalize(statement);
  return result;
}
