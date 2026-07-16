/* btech_persistence_sqlite.c -- BTech state in the MUX SQLite game database */

#include "mux/server/platform.h"

#include <errno.h>
#include <float.h>
#include <limits.h>
#include <math.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "glue.h"
#include "map.h"
#include "mech.events.h"
#include "mech.h"
#include "mech.tech.h"
#include "mechrep.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/server_api.h"
#include "mux/support/red_black_tree.h"
#include "p.mech.events.h"
#include "p.mech.utils.h"
#include "persistence/btech_persistence.h"
#include "turret.h"

extern RedBlackTree xcode_tree;
extern ACOM acom[AUTO_NUM_COMMANDS + 1];

/* Increment when a SQLite-only reader would need a compatibility change. */
#define BTECH_PERSISTENCE_SCHEMA_VERSION 1

/* Explicit map and repair-event tables are the first BTech SQLite mirror. */
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Woverlength-strings"
static const char btech_special_schema_sql[] =
    "CREATE TABLE btech_persistence_metadata ("
    " id INTEGER PRIMARY KEY CHECK (id = 1), schema_version INTEGER NOT NULL"
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
    " mech_dbref INTEGER PRIMARY KEY, pilot_status INTEGER NOT NULL, terrain "
    "INTEGER NOT NULL, elevation INTEGER NOT NULL,"
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

static void btech_special_test_reset_fault(void) {
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
static void btech_special_test_reset_fault(void) {}

static int btech_special_test_should_fail(const char *sql, const char *phase) {
  (void)sql;
  (void)phase;
  return 0;
}
#endif

/* Interpose only in this translation unit so prepare failures are testable. */
static int btech_special_prepare_v2(sqlite3 *sqlite, const char *sql,
                                    int byte_count, sqlite3_stmt **statement,
                                    const char **tail) {
  if (btech_special_test_should_fail(sql, "prepare")) {
    *statement = NULL;
    return SQLITE_ERROR;
  }
  return sqlite3_prepare_v2(sqlite, sql, byte_count, statement, tail);
}

#define sqlite3_prepare_v2 btech_special_prepare_v2

static int btech_special_exec(sqlite3 *sqlite, const char *sql) {
  char *error = NULL;
  int rc = sqlite3_exec(sqlite, sql, NULL, NULL, &error);

  sqlite3_free(error);
  return rc == SQLITE_OK ? 0 : -1;
}

static int btech_special_step(sqlite3_stmt *statement) {
  if (btech_special_test_should_fail(sqlite3_sql(statement), "step") ||
      sqlite3_step(statement) != SQLITE_DONE ||
      sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

static int btech_special_bind_int(sqlite3_stmt *statement, int index,
                                  sqlite3_int64 value) {
  return sqlite3_bind_int64(statement, index, value) == SQLITE_OK ? 0 : -1;
}

static int btech_special_bind_real(sqlite3_stmt *statement, int index,
                                   double value) {
  return sqlite3_bind_double(statement, index, value) == SQLITE_OK ? 0 : -1;
}

/* Mark the schema version in every snapshot, including snapshots with no BTech
 * objects. */
static int btech_special_store_metadata(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  int result;

  statement = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "INSERT INTO btech_persistence_metadata (id, schema_version) "
               "VALUES (1, ?);",
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
static int btech_special_column_int(sqlite3_stmt *statement, int column,
                                    int *value) {
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
static int btech_special_column_long(sqlite3_stmt *statement, int column,
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
static int btech_special_column_uint(sqlite3_stmt *statement, int column,
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
static int btech_special_column_ulong(sqlite3_stmt *statement, int column,
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

static int btech_special_column_char(sqlite3_stmt *statement, int column,
                                     char *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 ||
      number < CHAR_MIN || number > CHAR_MAX)
    return -1;
  *value = (char)number;
  return 0;
}

static int btech_special_column_uchar(sqlite3_stmt *statement, int column,
                                      unsigned char *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 || number < 0 ||
      number > UCHAR_MAX)
    return -1;
  *value = (unsigned char)number;
  return 0;
}

static int btech_special_column_short(sqlite3_stmt *statement, int column,
                                      short *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 ||
      number < SHRT_MIN || number > SHRT_MAX)
    return -1;
  *value = (short)number;
  return 0;
}

static int btech_special_column_ushort(sqlite3_stmt *statement, int column,
                                       unsigned short *value) {
  int number;

  if (btech_special_column_int(statement, column, &number) < 0 || number < 0 ||
      number > USHRT_MAX)
    return -1;
  *value = (unsigned short)number;
  return 0;
}

static int btech_special_column_dbref(sqlite3_stmt *statement, int column,
                                      DbRef *value) {
  if (btech_special_column_long(statement, column, value) < 0 ||
      (*value != NOTHING && !is_good_obj(*value)))
    return -1;
  return 0;
}

static int btech_special_column_time(sqlite3_stmt *statement, int column,
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
static int btech_special_column_real(sqlite3_stmt *statement, int column,
                                     float *value) {
  double number;
  int type;

  type = sqlite3_column_type(statement, column);
  if (type != SQLITE_FLOAT && type != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_double(statement, column);
  if (!isfinite(number) || number < -FLT_MAX || number > FLT_MAX)
    return -1;
  *value = (float)number;
  return 0;
}

/* Copy NUL-free SQLite text only when it fits the fixed BTech destination. */
static int btech_special_column_text(sqlite3_stmt *statement, int column,
                                     char *destination,
                                     size_t destination_size) {
  const unsigned char *text;
  int length;

  if (!destination_size ||
      sqlite3_column_type(statement, column) != SQLITE_TEXT)
    return -1;
  text = sqlite3_column_text(statement, column);
  length = sqlite3_column_bytes(statement, column);
  if (!text || length < 0 || (size_t)length >= destination_size ||
      (int)strlen((const char *)text) != length)
    return -1;
  memcpy(destination, text, (size_t)length + 1);
  return 0;
}

/* Require exactly one schema metadata row written by this persistence layer. */
static int btech_special_validate_metadata(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  int schema_version;
  int result;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT schema_version FROM btech_persistence_metadata "
          "WHERE id = 1;",
          -1, &statement, NULL) == SQLITE_OK &&
              sqlite3_step(statement) == SQLITE_ROW &&
              btech_special_column_int(statement, 0, &schema_version) == 0 &&
              schema_version == BTECH_PERSISTENCE_SCHEMA_VERSION &&
              sqlite3_step(statement) == SQLITE_DONE
          ? 0
          : -1;
  sqlite3_finalize(statement);
  return result;
}

typedef struct btech_map_store_context BTECH_MAP_STORE_CONTEXT;
struct btech_map_store_context {
  sqlite3_stmt *map;
  sqlite3_stmt *hex;
  sqlite3_stmt *slot;
  sqlite3_stmt *los;
  sqlite3_stmt *object;
  sqlite3_stmt *bits;
  int result;
};

typedef struct btech_object_store_context BTECH_OBJECT_STORE_CONTEXT;
struct btech_object_store_context {
  sqlite3_stmt *mechrep;
  sqlite3_stmt *turret;
  sqlite3_stmt *turret_tic;
  sqlite3_stmt *autopilot;
  sqlite3_stmt *mech;
  sqlite3_stmt *section;
  sqlite3_stmt *critical;
  sqlite3_stmt *position;
  sqlite3_stmt *bay;
  sqlite3_stmt *mech_turret;
  sqlite3_stmt *c3;
  sqlite3_stmt *c3node;
  sqlite3_stmt *tic;
  sqlite3_stmt *frequency;
  sqlite3_stmt *runtime;
  sqlite3_stmt *runtime_unused;
  sqlite3_stmt *unit_aux;
  sqlite3_stmt *stagger_damage;
  sqlite3_stmt *autopilot_command;
  sqlite3_stmt *autopilot_command_arg;
  sqlite3_stmt *autopilot_path;
  int result;
};

/* Finalize every object statement; SQLite permits finalizing NULL statements.
 */
static void
btech_finalize_object_statements(BTECH_OBJECT_STORE_CONTEXT *context) {
  sqlite3_finalize(context->mechrep);
  sqlite3_finalize(context->turret);
  sqlite3_finalize(context->turret_tic);
  sqlite3_finalize(context->autopilot);
  sqlite3_finalize(context->mech);
  sqlite3_finalize(context->section);
  sqlite3_finalize(context->critical);
  sqlite3_finalize(context->position);
  sqlite3_finalize(context->bay);
  sqlite3_finalize(context->mech_turret);
  sqlite3_finalize(context->c3);
  sqlite3_finalize(context->c3node);
  sqlite3_finalize(context->tic);
  sqlite3_finalize(context->frequency);
  sqlite3_finalize(context->runtime);
  sqlite3_finalize(context->runtime_unused);
  sqlite3_finalize(context->unit_aux);
  sqlite3_finalize(context->stagger_damage);
  sqlite3_finalize(context->autopilot_command);
  sqlite3_finalize(context->autopilot_command_arg);
  sqlite3_finalize(context->autopilot_path);
}

/* Replace the default map grid with a validated grid owned by this MAP. */
static int btech_special_resize_map(MAP *map, int width, int height) {
  unsigned char **grid;
  int y;

  if (width < 1 || width > MAPX || height < 1 || height > MAPY)
    return -1;
  grid = calloc((size_t)height, sizeof(*grid));
  if (!grid)
    return -1;
  for (y = 0; y < height; y++) {
    grid[y] = calloc((size_t)width, sizeof(*grid[y]));
    if (!grid[y]) {
      while (y-- > 0)
        free(grid[y]);
      free(grid);
      return -1;
    }
  }
  if (map->map) {
    for (y = 0; y < map->map_height; y++)
      free(map->map[y]);
    free(map->map);
  }
  map->map = grid;
  map->map_width = width;
  map->map_height = height;
  return 0;
}

/* Allocate the dynamic occupancy and LOS matrices after first_free is known. */
static int btech_special_allocate_map_dynamic(MAP *map) {
  int index;

  if (!map->first_free)
    return 0;
  map->mechsOnMap = calloc(map->first_free, sizeof(*map->mechsOnMap));
  map->mechflags = calloc(map->first_free, sizeof(*map->mechflags));
  map->LOSinfo = calloc(map->first_free, sizeof(*map->LOSinfo));
  for (index = 0; map->mechsOnMap && map->mechflags && map->LOSinfo &&
                  index < map->first_free;
       index++) {
    map->LOSinfo[index] = calloc(map->first_free, sizeof(*map->LOSinfo[index]));
  }
  if (map->mechsOnMap && map->mechflags && map->LOSinfo &&
      index == map->first_free)
    return 0;
  if (map->LOSinfo)
    for (index = 0; index < map->first_free; index++)
      free(map->LOSinfo[index]);
  free(map->LOSinfo);
  free(map->mechflags);
  free(map->mechsOnMap);
  map->LOSinfo = NULL;
  map->mechflags = NULL;
  map->mechsOnMap = NULL;
  return -1;
}

/* Restore map parent rows before any child row uses their dimensions. */
static int btech_special_load_map_parents(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MAP *map;
  char map_name[MAP_NAME_SIZE + 1];
  DbRef map_dbref;
  long long_value;
  int build_flag;
  int cf;
  int cf_max;
  int cloudbase;
  int first_free;
  int flags;
  int gravity;
  int height;
  int light;
  int max_visibility;
  int move_mod;
  int moves;
  int regen_factor;
  int reserved;
  int result;
  int sensor_flags;
  int step;
  int temperature;
  int visibility;
  int width;
  int wind_direction;
  int wind_speed;

  statement = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT dbref, map_name, width, height, temperature, gravity, "
               "cloudbase, visibility, max_visibility, light, wind_direction, "
               "wind_speed, reserved, flags, cf, cf_max, on_map, build_flag, "
               "first_free, moves, move_mod, sensor_flags, regen_factor "
               "FROM btech_maps ORDER BY dbref;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        !(map = getMap(map_dbref)) ||
        btech_special_column_text(statement, 1, map_name, sizeof(map_name)) <
            0 ||
        btech_special_column_int(statement, 2, &width) < 0 ||
        btech_special_column_int(statement, 3, &height) < 0 ||
        btech_special_column_int(statement, 4, &temperature) < 0 ||
        btech_special_column_int(statement, 5, &gravity) < 0 ||
        btech_special_column_int(statement, 6, &cloudbase) < 0 ||
        btech_special_column_int(statement, 7, &visibility) < 0 ||
        btech_special_column_int(statement, 8, &max_visibility) < 0 ||
        btech_special_column_int(statement, 9, &light) < 0 ||
        btech_special_column_int(statement, 10, &wind_direction) < 0 ||
        btech_special_column_int(statement, 11, &wind_speed) < 0 ||
        btech_special_column_int(statement, 12, &reserved) < 0 ||
        btech_special_column_int(statement, 13, &flags) < 0 ||
        btech_special_column_int(statement, 14, &cf) < 0 ||
        btech_special_column_int(statement, 15, &cf_max) < 0 ||
        btech_special_column_long(statement, 16, &long_value) < 0 ||
        btech_special_column_int(statement, 17, &build_flag) < 0 ||
        btech_special_column_int(statement, 18, &first_free) < 0 ||
        btech_special_column_int(statement, 19, &moves) < 0 ||
        btech_special_column_int(statement, 20, &move_mod) < 0 ||
        btech_special_column_int(statement, 21, &sensor_flags) < 0 ||
        btech_special_column_int(statement, 22, &regen_factor) < 0 ||
        temperature < CHAR_MIN || temperature > CHAR_MAX || gravity < 0 ||
        gravity > UCHAR_MAX || cloudbase < SHRT_MIN || cloudbase > SHRT_MAX ||
        visibility < CHAR_MIN || visibility > CHAR_MAX ||
        max_visibility < SHRT_MIN || max_visibility > SHRT_MAX ||
        light < CHAR_MIN || light > CHAR_MAX || wind_direction < SHRT_MIN ||
        wind_direction > SHRT_MAX || wind_speed < SHRT_MIN ||
        wind_speed > SHRT_MAX || reserved < CHAR_MIN || reserved > CHAR_MAX ||
        cf < SHRT_MIN || cf > SHRT_MAX || cf_max < SHRT_MIN ||
        cf_max > SHRT_MAX || build_flag < CHAR_MIN || build_flag > CHAR_MAX ||
        first_free < 0 || first_free > MAX_MECHS_PER_MAP || moves < SHRT_MIN ||
        moves > SHRT_MAX || move_mod < SHRT_MIN || move_mod > SHRT_MAX ||
        btech_special_resize_map(map, width, height) < 0) {
      result = -1;
      break;
    }
    memcpy(map->mapname, map_name, sizeof(map_name));
    map->temp = (char)temperature;
    map->grav = (unsigned char)gravity;
    map->cloudbase = (short)cloudbase;
    map->mapvis = (char)visibility;
    map->maxvis = (short)max_visibility;
    map->maplight = (char)light;
    map->winddir = (short)wind_direction;
    map->windspeed = (short)wind_speed;
    map->unused_char = (char)reserved;
    map->flags = flags;
    map->cf = (short)cf;
    map->cfmax = (short)cf_max;
    map->onmap = long_value;
    map->buildflag = (char)build_flag;
    map->first_free = (unsigned char)first_free;
    map->moves = (short)moves;
    map->movemod = (short)move_mod;
    map->sensorflags = sensor_flags;
    map->regen_factor = regen_factor;
    if (btech_special_allocate_map_dynamic(map) < 0) {
      result = -1;
      break;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore every base-grid byte in order, rejecting incomplete or sparse maps.
 */
static int btech_special_load_map_hexes(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MAP *map;
  DbRef current_map;
  DbRef map_dbref;
  int expected_x;
  int expected_y;
  int result;
  int step;
  int value;
  int x;
  int y;

  statement = NULL;
  current_map = NOTHING;
  expected_x = 0;
  expected_y = 0;
  map = NULL;
  result =
      sqlite3_prepare_v2(sqlite,
                         "SELECT map_dbref, x, y, value FROM btech_map_hexes "
                         "ORDER BY map_dbref, y, x;",
                         -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &x) < 0 ||
        btech_special_column_int(statement, 2, &y) < 0 ||
        btech_special_column_int(statement, 3, &value) < 0 || value < 0 ||
        value > UCHAR_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (map && expected_y != map->map_height) {
        result = -1;
        break;
      }
      map = getMap(map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      expected_x = 0;
      expected_y = 0;
    }
    if (x != expected_x || y != expected_y || y >= map->map_height) {
      result = -1;
      break;
    }
    map->map[y][x] = (unsigned char)value;
    if (++expected_x == map->map_width) {
      expected_x = 0;
      expected_y++;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && map && expected_y != map->map_height)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore occupancy rows into the dimensions allocated from each map parent. */
static int btech_special_load_map_slots(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MAP *map;
  DbRef current_map;
  DbRef map_dbref;
  long mech_dbref;
  int expected_slot;
  int flags;
  int result;
  int slot;
  int step;

  statement = NULL;
  current_map = NOTHING;
  expected_slot = 0;
  map = NULL;
  result = sqlite3_prepare_v2(sqlite,
                              "SELECT map_dbref, slot, mech_dbref, mech_flags "
                              "FROM btech_map_slots ORDER BY map_dbref, slot;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &slot) < 0 ||
        btech_special_column_long(statement, 2, &mech_dbref) < 0 ||
        btech_special_column_int(statement, 3, &flags) < 0 ||
        flags < CHAR_MIN || flags > CHAR_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (map && expected_slot != map->first_free) {
        result = -1;
        break;
      }
      map = getMap(map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      expected_slot = 0;
    }
    if (slot != expected_slot || slot >= map->first_free ||
        (mech_dbref != NOTHING && !getMech(mech_dbref))) {
      result = -1;
      break;
    }
    map->mechsOnMap[slot] = mech_dbref;
    map->mechflags[slot] = (char)flags;
    expected_slot++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && map && expected_slot != map->first_free)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore the complete square LOS matrix in its stable source/target order. */
static int btech_special_load_map_los(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MAP *map;
  DbRef current_map;
  DbRef map_dbref;
  int expected_source;
  int expected_target;
  int flags;
  int result;
  int source;
  int step;
  int target;

  statement = NULL;
  current_map = NOTHING;
  expected_source = 0;
  expected_target = 0;
  map = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT map_dbref, source_slot, target_slot, flags "
          "FROM btech_map_los ORDER BY map_dbref, source_slot, target_slot;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &source) < 0 ||
        btech_special_column_int(statement, 2, &target) < 0 ||
        btech_special_column_int(statement, 3, &flags) < 0 || flags < 0 ||
        flags > USHRT_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (map && expected_source != map->first_free) {
        result = -1;
        break;
      }
      map = getMap(map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      expected_source = 0;
      expected_target = 0;
    }
    if (source != expected_source || target != expected_target ||
        source >= map->first_free || target >= map->first_free) {
      result = -1;
      break;
    }
    map->LOSinfo[source][target] = (unsigned short)flags;
    if (++expected_target == map->first_free) {
      expected_target = 0;
      expected_source++;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && map && expected_source != map->first_free)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Ensure maps with zero children, and maps omitted from a child query, are
 * checked too. */
static int btech_special_validate_map_child_counts(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  int invalid_rows;
  int result;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT count(*) FROM btech_maps AS maps WHERE "
          "(SELECT count(*) FROM btech_map_hexes AS hexes "
          " WHERE hexes.map_dbref = maps.dbref) != maps.width * maps.height "
          "OR (SELECT count(*) FROM btech_map_slots AS slots "
          " WHERE slots.map_dbref = maps.dbref) != maps.first_free "
          "OR (SELECT count(*) FROM btech_map_los AS los "
          " WHERE los.map_dbref = maps.dbref) != maps.first_free * "
          "maps.first_free;",
          -1, &statement, NULL) == SQLITE_OK &&
              sqlite3_step(statement) == SQLITE_ROW &&
              btech_special_column_int(statement, 0, &invalid_rows) == 0 &&
              invalid_rows == 0 && sqlite3_step(statement) == SQLITE_DONE
          ? 0
          : -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore ordered map objects through the normal map-object allocator. */
static int btech_special_load_map_objects(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MAP *map;
  DbRef current_map;
  DbRef map_dbref;
  long data_int;
  long object_dbref;
  mapobj source;
  mapobj *stored;
  mapobj **tail;
  int current_object_type;
  int data_char;
  int data_short;
  int expected_ordinal;
  int object_type;
  int ordinal;
  int result;
  int step;
  int x;
  int y;

  statement = NULL;
  current_map = NOTHING;
  current_object_type = -1;
  object_type = -1;
  expected_ordinal = 0;
  map = NULL;
  tail = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT map_dbref, object_type, ordinal, x, y, object_dbref, "
               "data_char, data_short, data_int FROM btech_map_objects "
               "ORDER BY map_dbref, object_type, ordinal;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &object_type) < 0 ||
        btech_special_column_int(statement, 2, &ordinal) < 0 ||
        btech_special_column_int(statement, 3, &x) < 0 ||
        btech_special_column_int(statement, 4, &y) < 0 ||
        btech_special_column_long(statement, 5, &object_dbref) < 0 ||
        btech_special_column_int(statement, 6, &data_char) < 0 ||
        btech_special_column_int(statement, 7, &data_short) < 0 ||
        btech_special_column_long(statement, 8, &data_int) < 0 ||
        object_type < 0 || object_type >= NUM_MAPOBJTYPES ||
        object_type == TYPE_BITS || ordinal < 0 || x < SHRT_MIN ||
        x > SHRT_MAX || y < SHRT_MIN || y > SHRT_MAX || data_short < SHRT_MIN ||
        data_short > SHRT_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map || object_type != current_object_type) {
      map = getMap(map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      current_object_type = object_type;
      expected_ordinal = 0;
      tail = &map->mapobj[object_type];
      source.type = (char)object_type;
    }
    if (ordinal != expected_ordinal || x < 0 || x >= map->map_width || y < 0 ||
        y >= map->map_height ||
        (object_dbref != NOTHING && !is_good_obj(object_dbref))) {
      result = -1;
      break;
    }
    memset(&source, 0, sizeof(source));
    source.type = (char)object_type;
    source.x = (short)x;
    source.y = (short)y;
    source.obj = object_dbref;
    source.datac = data_char;
    source.datas = (short)data_short;
    source.datai = data_int;
    stored = add_mapobj(map, tail, &source, 0);
    if (!stored) {
      result = -1;
      break;
    }
    tail = &stored->next;
    expected_ordinal++;
    if (object_type == TYPE_BUILD)
      possibly_start_building_regen(source.obj);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Rebuild the TYPE_BITS allocation without ever serializing its pointer. */
static int btech_special_load_map_bits(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MAP *map;
  DbRef current_map;
  DbRef map_dbref;
  mapobj source;
  unsigned char **bits;
  int bytes_per_row = 0;
  int current_y;
  int expected_byte;
  int result;
  int step;
  int value;
  int byte_index;
  int y;

  statement = NULL;
  current_map = NOTHING;
  current_y = -1;
  expected_byte = 0;
  map = NULL;
  bits = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT map_dbref, y, byte_index, value FROM btech_map_bits "
               "ORDER BY map_dbref, y, byte_index;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &y) < 0 ||
        btech_special_column_int(statement, 2, &byte_index) < 0 ||
        btech_special_column_int(statement, 3, &value) < 0 || value < 0 ||
        value > UCHAR_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (current_y >= 0 && expected_byte != bytes_per_row) {
        result = -1;
        break;
      }
      map = getMap(map_dbref);
      if (!map || map->mapobj[TYPE_BITS]) {
        result = -1;
        break;
      }
      bits = calloc((size_t)map->map_height, sizeof(*bits));
      if (!bits) {
        result = -1;
        break;
      }
      memset(&source, 0, sizeof(source));
      source.type = TYPE_BITS;
      source.datai = (long)(void *)bits;
      if (!add_mapobj(map, &map->mapobj[TYPE_BITS], &source, 0)) {
        free(bits);
        result = -1;
        break;
      }
      current_map = map_dbref;
      current_y = -1;
      expected_byte = 0;
    }
    bytes_per_row = map->map_width / 4 + (map->map_width % 4 ? 1 : 0);
    if (y < 0 || y >= map->map_height || byte_index < 0 ||
        byte_index >= bytes_per_row || (current_y >= 0 && y < current_y) ||
        (y == current_y && byte_index != expected_byte)) {
      result = -1;
      break;
    }
    if (y != current_y) {
      if (current_y >= 0 && expected_byte != bytes_per_row) {
        result = -1;
        break;
      }
      bits[y] = calloc((size_t)bytes_per_row, sizeof(*bits[y]));
      if (!bits[y]) {
        result = -1;
        break;
      }
      current_y = y;
      expected_byte = 0;
    }
    bits[y][byte_index] = (unsigned char)value;
    expected_byte++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && current_y >= 0 && expected_byte != bytes_per_row)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Map each persisted repair type to the canonical repair completion callback.
 */
static void (*btech_special_repair_function(int type))(MuxEvent *) {
  switch (type) {
  case EVENT_REPAIR_MOB:
    return mux_event_tickmech_mountbomb;
  case EVENT_REPAIR_UMOB:
    return mux_event_tickmech_umountbomb;
  case EVENT_REPAIR_REPL:
  case EVENT_REPAIR_REPAP:
    return mux_event_tickmech_repairpart;
  case EVENT_REPAIR_REPLG:
    return mux_event_tickmech_replacegun;
  case EVENT_REPAIR_REPENHCRIT:
    return mux_event_tickmech_repairenhcrit;
  case EVENT_REPAIR_REPAG:
    return mux_event_tickmech_repairgun;
  case EVENT_REPAIR_REAT:
    return mux_event_tickmech_reattach;
  case EVENT_REPAIR_RELO:
    return mux_event_tickmech_reload;
  case EVENT_REPAIR_FIX:
    return mux_event_tickmech_repairarmor;
  case EVENT_REPAIR_FIXI:
    return mux_event_tickmech_repairinternal;
  case EVENT_REPAIR_SCRL:
    return mux_event_tickmech_removesection;
  case EVENT_REPAIR_SCRG:
    return mux_event_tickmech_removegun;
  case EVENT_REPAIR_SCRP:
    return mux_event_tickmech_removepart;
  case EVENT_REPAIR_RESE:
    return mux_event_tickmech_reseal;
  case EVENT_REPAIR_REPSUIT:
    return mux_event_tickmech_replacesuit;
  default:
    return NULL;
  }
}

/* Requeue repair work with its original remaining ticks and fake-event state.
 */
static int btech_special_load_repair_events(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
  DbRef mech_dbref;
  long event_data;
  void (*function)(MuxEvent *);
  int event_type;
  int fake;
  int remaining_ticks;
  int result;
  int step;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT mech_dbref, event_type, remaining_ticks, event_data, is_fake "
          "FROM btech_repair_events ORDER BY event_id;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        !(mech = getMech(mech_dbref)) ||
        btech_special_column_int(statement, 1, &event_type) < 0 ||
        btech_special_column_int(statement, 2, &remaining_ticks) < 0 ||
        btech_special_column_long(statement, 3, &event_data) < 0 ||
        btech_special_column_int(statement, 4, &fake) < 0 ||
        event_type < FIRST_TECH_EVENT || event_type > LAST_TECH_EVENT ||
        remaining_ticks < 1 || fake < 0 || fake > 1) {
      result = -1;
      break;
    }
    function =
        fake ? very_fake_func : btech_special_repair_function(event_type);
    if (!function) {
      result = -1;
      break;
    }
    mux_event_add(remaining_ticks, 0, event_type, function, mech,
                  (void *)event_data);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore MECH identity and unit-definition fields before child tables. */
static int btech_special_load_mech_parents(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
  char mech_name[sizeof(mech->ud.mech_name)];
  char mech_type[sizeof(mech->ud.mech_type)];
  char unit_era[sizeof(mech->ud.unit_era)];
  char unit_tro[sizeof(mech->ud.unit_tro)];
  DbRef mech_dbref;
  long map_dbref;
  float max_speed;
  float template_max_speed;
  int battle_value;
  int cargo_space;
  int carrier_max_tons;
  int computer;
  int fuel;
  int fuel_original;
  int heat_sink_override;
  int heat_sinks;
  int id_0;
  int id_1;
  int lrs_range;
  int map_number;
  int movement_type;
  int radio;
  int radio_info;
  int radio_range;
  int result;
  int run_speed;
  int scan_range;
  int step;
  int structural_integrity;
  int structural_integrity_original;
  int tactical_range;
  int targeting_computer;
  int tons;
  int unit_class;
  int walk_speed;
  int brief;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT dbref, id_0, id_1, brief, map_number, map_dbref, "
          "mech_name, mech_type, unit_era, unit_tro, unit_class, "
          "movement_type, tactical_range, lrs_range, scan_range, heat_sinks, "
          "heat_sink_override, computer, radio, radio_info, "
          "structural_integrity, structural_integrity_original, radio_range, "
          "fuel, fuel_original, tons, walk_speed, run_speed, max_speed, "
          "template_max_speed, battle_value, cargo_space, targeting_computer, "
          "carrier_max_tons FROM btech_mechs ORDER BY dbref;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        !(mech = getMech(mech_dbref)) ||
        btech_special_column_int(statement, 1, &id_0) < 0 ||
        btech_special_column_int(statement, 2, &id_1) < 0 ||
        btech_special_column_int(statement, 3, &brief) < 0 ||
        btech_special_column_int(statement, 4, &map_number) < 0 ||
        btech_special_column_long(statement, 5, &map_dbref) < 0 ||
        btech_special_column_text(statement, 6, mech_name, sizeof(mech_name)) <
            0 ||
        btech_special_column_text(statement, 7, mech_type, sizeof(mech_type)) <
            0 ||
        btech_special_column_text(statement, 8, unit_era, sizeof(unit_era)) <
            0 ||
        btech_special_column_text(statement, 9, unit_tro, sizeof(unit_tro)) <
            0 ||
        btech_special_column_int(statement, 10, &unit_class) < 0 ||
        btech_special_column_int(statement, 11, &movement_type) < 0 ||
        btech_special_column_int(statement, 12, &tactical_range) < 0 ||
        btech_special_column_int(statement, 13, &lrs_range) < 0 ||
        btech_special_column_int(statement, 14, &scan_range) < 0 ||
        btech_special_column_int(statement, 15, &heat_sinks) < 0 ||
        btech_special_column_int(statement, 16, &heat_sink_override) < 0 ||
        btech_special_column_int(statement, 17, &computer) < 0 ||
        btech_special_column_int(statement, 18, &radio) < 0 ||
        btech_special_column_int(statement, 19, &radio_info) < 0 ||
        btech_special_column_int(statement, 20, &structural_integrity) < 0 ||
        btech_special_column_int(statement, 21,
                                 &structural_integrity_original) < 0 ||
        btech_special_column_int(statement, 22, &radio_range) < 0 ||
        btech_special_column_int(statement, 23, &fuel) < 0 ||
        btech_special_column_int(statement, 24, &fuel_original) < 0 ||
        btech_special_column_int(statement, 25, &tons) < 0 ||
        btech_special_column_int(statement, 26, &walk_speed) < 0 ||
        btech_special_column_int(statement, 27, &run_speed) < 0 ||
        btech_special_column_real(statement, 28, &max_speed) < 0 ||
        btech_special_column_real(statement, 29, &template_max_speed) < 0 ||
        btech_special_column_int(statement, 30, &battle_value) < 0 ||
        btech_special_column_int(statement, 31, &cargo_space) < 0 ||
        btech_special_column_int(statement, 32, &targeting_computer) < 0 ||
        btech_special_column_int(statement, 33, &carrier_max_tons) < 0 ||
        id_0 < CHAR_MIN || id_0 > CHAR_MAX || id_1 < CHAR_MIN ||
        id_1 > CHAR_MAX || brief < CHAR_MIN || brief > CHAR_MAX ||
        unit_class < CHAR_MIN || unit_class > CHAR_MAX ||
        movement_type < CHAR_MIN || movement_type > CHAR_MAX ||
        tactical_range < CHAR_MIN || tactical_range > CHAR_MAX ||
        lrs_range < CHAR_MIN || lrs_range > CHAR_MAX || scan_range < CHAR_MIN ||
        scan_range > CHAR_MAX || heat_sinks < CHAR_MIN ||
        heat_sinks > CHAR_MAX || computer < CHAR_MIN || computer > CHAR_MAX ||
        radio < CHAR_MIN || radio > CHAR_MAX || radio_info < 0 ||
        radio_info > UCHAR_MAX || structural_integrity < CHAR_MIN ||
        structural_integrity > CHAR_MAX ||
        structural_integrity_original < CHAR_MIN ||
        structural_integrity_original > CHAR_MAX || radio_range < SHRT_MIN ||
        radio_range > SHRT_MAX || targeting_computer < CHAR_MIN ||
        targeting_computer > CHAR_MAX || carrier_max_tons < CHAR_MIN ||
        carrier_max_tons > CHAR_MAX ||
        (map_dbref != NOTHING && !getMap(map_dbref))) {
      result = -1;
      break;
    }
    mech->ID[0] = (char)id_0;
    mech->ID[1] = (char)id_1;
    mech->brief = (char)brief;
    mech->mapnumber = map_number;
    mech->mapindex = map_dbref;
    memcpy(mech->ud.mech_name, mech_name, sizeof(mech_name));
    memcpy(mech->ud.mech_type, mech_type, sizeof(mech_type));
    memcpy(mech->ud.unit_era, unit_era, sizeof(unit_era));
    memcpy(mech->ud.unit_tro, unit_tro, sizeof(unit_tro));
    mech->ud.type = (char)unit_class;
    mech->ud.move = (char)movement_type;
    mech->ud.tac_range = (char)tactical_range;
    mech->ud.lrs_range = (char)lrs_range;
    mech->ud.scan_range = (char)scan_range;
    mech->ud.numsinks = (char)heat_sinks;
    mech->ud.hsengoverride = heat_sink_override;
    mech->ud.computer = (char)computer;
    mech->ud.radio = (char)radio;
    mech->ud.radioinfo = (unsigned char)radio_info;
    mech->ud.si = (char)structural_integrity;
    mech->ud.si_orig = (char)structural_integrity_original;
    mech->ud.radio_range = (short)radio_range;
    mech->ud.fuel = fuel;
    mech->ud.fuel_orig = fuel_original;
    mech->ud.tons = tons;
    mech->ud.walkspeed = walk_speed;
    mech->ud.runspeed = run_speed;
    mech->ud.maxspeed = max_speed;
    mech->ud.template_maxspeed = template_max_speed;
    mech->ud.mechbv = battle_value;
    mech->ud.cargospace = cargo_space;
    mech->ud.targcomp = (char)targeting_computer;
    mech->ud.carmaxton = (char)carrier_max_tons;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore every section row in stable section-index order. */
static int btech_special_load_mech_sections(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
  DbRef current_mech;
  DbRef mech_dbref;
  struct section_struct *section;
  int armor;
  int armor_original;
  int base_to_hit;
  int config;
  int expected_section;
  int internal;
  int internal_original;
  int rear;
  int rear_original;
  int recycle;
  int result;
  int section_index;
  int specials;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  expected_section = 0;
  mech = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT mech_dbref, section, armor, internal, rear, armor_original, "
          "internal_original, rear_original, base_to_hit, config, recycle, "
          "specials "
          "FROM btech_mech_sections ORDER BY mech_dbref, section;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &section_index) < 0 ||
        btech_special_column_int(statement, 2, &armor) < 0 ||
        btech_special_column_int(statement, 3, &internal) < 0 ||
        btech_special_column_int(statement, 4, &rear) < 0 ||
        btech_special_column_int(statement, 5, &armor_original) < 0 ||
        btech_special_column_int(statement, 6, &internal_original) < 0 ||
        btech_special_column_int(statement, 7, &rear_original) < 0 ||
        btech_special_column_int(statement, 8, &base_to_hit) < 0 ||
        btech_special_column_int(statement, 9, &config) < 0 ||
        btech_special_column_int(statement, 10, &recycle) < 0 ||
        btech_special_column_int(statement, 11, &specials) < 0 || armor < 0 ||
        armor > UCHAR_MAX || internal < 0 || internal > UCHAR_MAX || rear < 0 ||
        rear > UCHAR_MAX || armor_original < 0 || armor_original > UCHAR_MAX ||
        internal_original < 0 || internal_original > UCHAR_MAX ||
        rear_original < 0 || rear_original > UCHAR_MAX ||
        base_to_hit < CHAR_MIN || base_to_hit > CHAR_MAX || config < CHAR_MIN ||
        config > CHAR_MAX || recycle < CHAR_MIN || recycle > CHAR_MAX ||
        specials < 0 || specials > USHRT_MAX) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_section != NUM_SECTIONS) {
        result = -1;
        break;
      }
      mech = getMech(mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_section = 0;
    }
    if (section_index != expected_section || section_index >= NUM_SECTIONS) {
      result = -1;
      break;
    }
    section = &mech->ud.sections[section_index];
    section->armor = (unsigned char)armor;
    section->internal = (unsigned char)internal;
    section->rear = (unsigned char)rear;
    section->armor_orig = (unsigned char)armor_original;
    section->internal_orig = (unsigned char)internal_original;
    section->rear_orig = (unsigned char)rear_original;
    section->basetohit = (char)base_to_hit;
    section->config = (char)config;
    section->recycle = (char)recycle;
    section->specials = (unsigned short)specials;
    expected_section++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && expected_section != NUM_SECTIONS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all twelve critical slots per section without restoring pointers. */
static int btech_special_load_mech_criticals(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
  DbRef current_mech;
  DbRef mech_dbref;
  struct critical_slot *critical;
  unsigned int ammo_mode;
  int brand;
  int current_section;
  int data;
  unsigned int damage_flags;
  int desired_ammo_location;
  int expected_slot;
  unsigned int fire_mode;
  int item_type;
  int result;
  int section_index;
  int slot;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  current_section = -1;
  expected_slot = 0;
  mech = NULL;
  result = sqlite3_prepare_v2(sqlite,
                              "SELECT mech_dbref, section, slot, brand, data, "
                              "item_type, fire_mode, "
                              "ammo_mode, damage_flags, desired_ammo_location "
                              "FROM btech_mech_criticals "
                              "ORDER BY mech_dbref, section, slot;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &section_index) < 0 ||
        btech_special_column_int(statement, 2, &slot) < 0 ||
        btech_special_column_int(statement, 3, &brand) < 0 ||
        btech_special_column_int(statement, 4, &data) < 0 ||
        btech_special_column_int(statement, 5, &item_type) < 0 ||
        btech_special_column_uint(statement, 6, &fire_mode) < 0 ||
        btech_special_column_uint(statement, 7, &ammo_mode) < 0 ||
        btech_special_column_uint(statement, 8, &damage_flags) < 0 ||
        btech_special_column_int(statement, 9, &desired_ammo_location) < 0 ||
        brand < 0 || brand > UCHAR_MAX || data < 0 || data > UCHAR_MAX ||
        item_type < 0 || item_type > USHRT_MAX ||
        desired_ammo_location < SHRT_MIN || desired_ammo_location > SHRT_MAX) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && (current_section != NUM_SECTIONS - 1 ||
                   expected_slot != NUM_CRITICALS)) {
        result = -1;
        break;
      }
      mech = getMech(mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      current_section = 0;
      expected_slot = 0;
    }
    if (section_index != current_section) {
      if (expected_slot != NUM_CRITICALS ||
          section_index != current_section + 1) {
        result = -1;
        break;
      }
      current_section = section_index;
      expected_slot = 0;
    }
    if (section_index < 0 || section_index >= NUM_SECTIONS ||
        slot != expected_slot || slot >= NUM_CRITICALS) {
      result = -1;
      break;
    }
    critical = &mech->ud.sections[section_index].criticals[slot];
    critical->brand = (unsigned char)brand;
    critical->data = (unsigned char)data;
    critical->type = (unsigned short)item_type;
    critical->firemode = fire_mode;
    critical->ammomode = ammo_mode;
    critical->weapDamageFlags = damage_flags;
    critical->desiredAmmoLoc = (short)desired_ammo_location;
    expected_slot++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech &&
      (current_section != NUM_SECTIONS - 1 || expected_slot != NUM_CRITICALS))
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore the non-pointer MECH position record. */
static int btech_special_load_mech_positions(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
  DbRef mech_dbref;
  long pilot;
  float fx;
  float fy;
  float fz;
  float hexes_walked;
  int elevation;
  int facing;
  int last_x;
  int last_y;
  int pilot_status;
  int result;
  int stall;
  int step;
  int team;
  int terrain;
  int unusable_arcs;
  int x;
  int y;
  int z;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT mech_dbref, pilot_status, terrain, elevation, hexes_walked, "
          "facing, x, y, z, last_x, last_y, fx, fy, fz, team, unusable_arcs, "
          "stall, pilot FROM btech_mech_positions ORDER BY mech_dbref;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        !(mech = getMech(mech_dbref)) ||
        btech_special_column_int(statement, 1, &pilot_status) < 0 ||
        btech_special_column_int(statement, 2, &terrain) < 0 ||
        btech_special_column_int(statement, 3, &elevation) < 0 ||
        btech_special_column_real(statement, 4, &hexes_walked) < 0 ||
        btech_special_column_int(statement, 5, &facing) < 0 ||
        btech_special_column_int(statement, 6, &x) < 0 ||
        btech_special_column_int(statement, 7, &y) < 0 ||
        btech_special_column_int(statement, 8, &z) < 0 ||
        btech_special_column_int(statement, 9, &last_x) < 0 ||
        btech_special_column_int(statement, 10, &last_y) < 0 ||
        btech_special_column_real(statement, 11, &fx) < 0 ||
        btech_special_column_real(statement, 12, &fy) < 0 ||
        btech_special_column_real(statement, 13, &fz) < 0 ||
        btech_special_column_int(statement, 14, &team) < 0 ||
        btech_special_column_int(statement, 15, &unusable_arcs) < 0 ||
        btech_special_column_int(statement, 16, &stall) < 0 ||
        btech_special_column_long(statement, 17, &pilot) < 0 ||
        pilot_status < CHAR_MIN || pilot_status > CHAR_MAX ||
        terrain < CHAR_MIN || terrain > CHAR_MAX || elevation < CHAR_MIN ||
        elevation > CHAR_MAX || facing < SHRT_MIN || facing > SHRT_MAX ||
        x < SHRT_MIN || x > SHRT_MAX || y < SHRT_MIN || y > SHRT_MAX ||
        z < SHRT_MIN || z > SHRT_MAX || last_x < SHRT_MIN ||
        last_x > SHRT_MAX || last_y < SHRT_MIN || last_y > SHRT_MAX ||
        (pilot != NOTHING && !is_good_obj(pilot))) {
      result = -1;
      break;
    }
    mech->pd.pilotstatus = (char)pilot_status;
    mech->pd.terrain = (char)terrain;
    mech->pd.elev = (char)elevation;
    mech->pd.hexes_walked = hexes_walked;
    mech->pd.facing = (short)facing;
    mech->pd.x = (short)x;
    mech->pd.y = (short)y;
    mech->pd.z = (short)z;
    mech->pd.last_x = (short)last_x;
    mech->pd.last_y = (short)last_y;
    mech->pd.fx = fx;
    mech->pd.fy = fy;
    mech->pd.fz = fz;
    mech->pd.team = team;
    mech->pd.unusable_arcs = unusable_arcs;
    mech->pd.stall = stall;
    mech->pd.pilot = pilot;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all bay dbref links in their fixed four-slot order. */
static int btech_special_load_mech_bays(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
        (bay_dbref != NOTHING && !is_good_obj(bay_dbref))) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_bay != NUM_BAYS) {
        result = -1;
        break;
      }
      mech = getMech(mech_dbref);
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
    mech->pd.bay[bay_index] = bay_dbref;
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
static int btech_special_load_mech_turrets(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
        (turret_dbref != NOTHING && !is_good_obj(turret_dbref))) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_turret != NUM_TURRETS) {
        result = -1;
        break;
      }
      mech = getMech(mech_dbref);
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
    mech->pd.turret[turret_index] = turret_dbref;
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
static int btech_special_load_mech_c3(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
  char channel_title[sizeof(mech->sd.C3ChanTitle)];
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
        !(mech = getMech(mech_dbref)) ||
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
        (tag_target != NOTHING && !is_good_obj(tag_target)) ||
        (tagged_by != NOTHING && !is_good_obj(tagged_by))) {
      result = -1;
      break;
    }
    memcpy(mech->sd.C3ChanTitle, channel_title, sizeof(channel_title));
    mech->sd.wC3iNetworkSize = c3i_size;
    mech->sd.wC3NetworkSize = c3_size;
    mech->sd.wTotalC3Masters = total_masters;
    mech->sd.wWorkingC3Masters = working_masters;
    mech->sd.C3FreqMode = frequency_mode;
    mech->sd.tagTarget = tag_target;
    mech->sd.taggedBy = tagged_by;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore every C3i and C3 array element, including empty slots. */
static int btech_special_load_mech_c3_nodes(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
        (node_dbref != NOTHING && node_dbref != 0 && !getMech(node_dbref))) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && (expected_network != 2 || expected_node != 0)) {
        result = -1;
        break;
      }
      mech = getMech(mech_dbref);
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
    if (network_type == 0)
      mech->sd.C3iNetwork[node_index] = node_dbref;
    else
      mech->sd.C3Network[node_index] = node_dbref;
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
static int btech_special_load_mech_tics(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
      mech = getMech(mech_dbref);
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
    mech->tic[tic_index][word_index] = value;
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
static int btech_special_load_mech_frequencies(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
      mech = getMech(mech_dbref);
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
    mech->freq[frequency_index] = frequency;
    mech->freqmodes[frequency_index] = mode;
    memcpy(mech->chantitle[frequency_index], title, sizeof(title));
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
static int btech_special_load_mech_runtime(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
        !(mech = getMech(mech_dbref))) {
      result = -1;
      break;
    }
#define RUNTIME_CHAR(column, field)                                            \
  btech_special_column_char(statement, column, &mech->rd.field)
#define RUNTIME_SHORT(column, field)                                           \
  btech_special_column_short(statement, column, &mech->rd.field)
#define RUNTIME_INT(column, field)                                             \
  btech_special_column_int(statement, column, &mech->rd.field)
#define RUNTIME_REAL(column, field)                                            \
  btech_special_column_real(statement, column, &mech->rd.field)
#define RUNTIME_DBREF(column, field)                                           \
  btech_special_column_dbref(statement, column, &mech->rd.field)
    if (RUNTIME_CHAR(1, jumptop) < 0 || RUNTIME_CHAR(2, aim) < 0 ||
        RUNTIME_CHAR(3, basetohit) < 0 || RUNTIME_CHAR(4, pilotskillbase) < 0 ||
        RUNTIME_CHAR(5, engineheat) < 0 || RUNTIME_CHAR(6, masc_value) < 0 ||
        RUNTIME_CHAR(7, aim_type) < 0 ||
        btech_special_column_char(statement, 8, &mech->rd.sensor[0]) < 0 ||
        btech_special_column_char(statement, 9, &mech->rd.sensor[1]) < 0 ||
        btech_special_column_uchar(statement, 10, &mech->rd.fire_adjustment) <
            0 ||
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
                                  &mech->rd.last_weapon_recycle) < 0 ||
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
        btech_special_column_time(statement, 94, &mech->rd.lastStaggerCheck) <
            0)
      result = -1;
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
static int btech_special_load_mech_unit_aux(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
      mech = getMech(mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      memset(seen, 0, sizeof(seen));
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
      mech->ud.unused[slot] = value;
    else if (value < CHAR_MIN || value > CHAR_MAX)
      result = -1;
    else
      mech->ud.unused_char[slot - 8] = (char)value;
#else
    if (slot == 0)
      mech->ud.mechbv_last = value;
    else if (value < CHAR_MIN || value > CHAR_MAX)
      result = -1;
    else
      mech->ud.unused_char[slot - 8] = (char)value;
#endif
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
static int btech_special_load_mech_runtime_unused(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
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
      mech = getMech(mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_slot = 0;
    }
    if (slot != expected_slot || slot >= 5) {
      result = -1;
      break;
    }
    mech->rd.unused[slot] = value;
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
static int btech_special_load_mech_stagger_damage(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECH *mech;
  damageNode *node;
  damageNode *tail;
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
  tail = NULL;
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
        btech_special_column_dbref(statement, 4, &attacker) < 0 ||
        btech_special_column_int(statement, 5, &counted) < 0 || counted < 0 ||
        counted > 1) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      mech = getMech(mech_dbref);
      if (!mech || mech->rd.staggerDamageList) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_position = 0;
      tail = NULL;
    }
    if (position != expected_position) {
      result = -1;
      break;
    }
    node = calloc(1, sizeof(*node));
    if (!node) {
      result = -1;
      break;
    }
    node->amount = amount;
    node->occuredAt = occurred_at;
    node->attackerNum = attacker;
    node->counted = counted;
    if (tail)
      tail->next = node;
    else
      mech->rd.staggerDamageList = node;
    tail = node;
    expected_position++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Resolve a preallocated special object and reject a row of the wrong type. */
static void *btech_special_object(DbRef object, GlueType type) {
  if (!is_good_obj(object) || WhichSpecial(object) != (int)type)
    return NULL;
  return FindObjectsData(object);
}

/* Restore repair-console target rows. */
static int btech_special_load_mechrep(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  MECHREP *mechrep;
  DbRef object;
  DbRef target;
  int result;
  int step;

  statement = NULL;
  result = sqlite3_prepare_v2(sqlite,
                              "SELECT dbref, current_target FROM btech_mechrep "
                              "ORDER BY dbref;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &object) < 0 ||
        !(mechrep = btech_special_object(object, GTYPE_MECHREP)) ||
        btech_special_column_dbref(statement, 1, &target) < 0)
      result = -1;
    else
      mechrep->current_target = target;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore a turret parent and every independent timing slot. */
static int btech_special_load_turrets(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  TURRET_T *turret;
  DbRef object;
  DbRef parent;
  DbRef gunner;
  DbRef target;
  int arcs;
  int lock_mode;
  int result;
  int step;
  int target_x;
  int target_y;
  int target_z;

  statement = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT dbref, arcs, parent, gunner, target, target_x, target_y, "
          "target_z, lock_mode FROM btech_turrets ORDER BY dbref;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &object) < 0 ||
        !(turret = btech_special_object(object, GTYPE_TURRET)) ||
        btech_special_column_int(statement, 1, &arcs) < 0 ||
        btech_special_column_dbref(statement, 2, &parent) < 0 ||
        btech_special_column_dbref(statement, 3, &gunner) < 0 ||
        btech_special_column_dbref(statement, 4, &target) < 0 ||
        btech_special_column_int(statement, 5, &target_x) < 0 ||
        btech_special_column_int(statement, 6, &target_y) < 0 ||
        btech_special_column_int(statement, 7, &target_z) < 0 ||
        btech_special_column_int(statement, 8, &lock_mode) < 0) {
      result = -1;
      break;
    }
    turret->arcs = arcs;
    turret->parent = parent;
    turret->gunner = gunner;
    turret->target = target;
    turret->targx = target_x;
    turret->targy = target_y;
    turret->targz = target_z;
    turret->lockmode = lock_mode;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all NUM_TICS turret timing values in fixed index order. */
static int btech_special_load_turret_tics(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  TURRET_T *turret;
  DbRef current_turret;
  DbRef turret_dbref;
  int expected_tic;
  int result;
  int step;
  int tic_index;
  int value;

  statement = NULL;
  current_turret = NOTHING;
  expected_tic = 0;
  turret = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT turret_dbref, tic_index, value FROM btech_turret_tics "
               "ORDER BY turret_dbref, tic_index;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &turret_dbref) < 0 ||
        turret_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &tic_index) < 0 ||
        btech_special_column_int(statement, 2, &value) < 0) {
      result = -1;
      break;
    }
    if (turret_dbref != current_turret) {
      if (turret && expected_tic != NUM_TICS) {
        result = -1;
        break;
      }
      turret = btech_special_object(turret_dbref, GTYPE_TURRET);
      if (!turret) {
        result = -1;
        break;
      }
      current_turret = turret_dbref;
      expected_tic = 0;
    }
    if (tic_index != expected_tic || tic_index >= NUM_TICS) {
      result = -1;
      break;
    }
    turret->tic[tic_index] = value;
    expected_tic++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && turret && expected_tic != NUM_TICS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore AUTOPILOT scalar state; command and path lists are loaded separately.
 */
static int btech_special_load_autopilots(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  AUTO *autopilot;
  DbRef object;
  DbRef map_dbref;
  DbRef mech_dbref;
  DbRef target;
  int result;
  int step;

  statement = NULL;
  result = sqlite3_prepare_v2(sqlite,
                              "SELECT * FROM btech_autopilots ORDER BY dbref;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &object) < 0 ||
        !(autopilot = btech_special_object(object, GTYPE_AUTO)) ||
        btech_special_column_dbref(statement, 1, &mech_dbref) < 0 ||
        btech_special_column_dbref(statement, 2, &map_dbref) < 0 ||
        btech_special_column_ushort(statement, 3, &autopilot->speed) < 0 ||
        btech_special_column_int(statement, 4, &autopilot->ofsx) < 0 ||
        btech_special_column_int(statement, 5, &autopilot->ofsy) < 0 ||
        btech_special_column_uchar(statement, 6, &autopilot->verbose_level) <
            0 ||
        btech_special_column_dbref(statement, 7, &target) < 0 ||
        btech_special_column_int(statement, 8, &autopilot->target_score) < 0 ||
        btech_special_column_int(statement, 9, &autopilot->target_threshold) <
            0 ||
        btech_special_column_int(statement, 10,
                                 &autopilot->target_update_tick) < 0 ||
        btech_special_column_dbref(statement, 11, &autopilot->chase_target) <
            0 ||
        btech_special_column_int(statement, 12,
                                 &autopilot->chasetarg_update_tick) < 0 ||
        btech_special_column_int(statement, 13,
                                 &autopilot->follow_update_tick) < 0 ||
        btech_special_column_ushort(statement, 14, &autopilot->flags) < 0 ||
        btech_special_column_int(statement, 15, &autopilot->mech_max_range) <
            0 ||
        btech_special_column_uchar(statement, 16, &autopilot->roam_type) < 0 ||
        btech_special_column_int(statement, 17, &autopilot->roam_update_tick) <
            0 ||
        btech_special_column_short(statement, 18,
                                   &autopilot->roam_target_hex_x) < 0 ||
        btech_special_column_short(statement, 19,
                                   &autopilot->roam_target_hex_y) < 0 ||
        btech_special_column_short(statement, 20,
                                   &autopilot->roam_anchor_hex_x) < 0 ||
        btech_special_column_short(statement, 21,
                                   &autopilot->roam_anchor_hex_y) < 0 ||
        btech_special_column_short(statement, 22,
                                   &autopilot->roam_anchor_distance) < 0 ||
        btech_special_column_int(statement, 23, &autopilot->ahead_ok) < 0 ||
        btech_special_column_int(statement, 24, &autopilot->auto_cmode) < 0 ||
        btech_special_column_int(statement, 25, &autopilot->auto_cdist) < 0 ||
        btech_special_column_int(statement, 26, &autopilot->auto_goweight) <
            0 ||
        btech_special_column_int(statement, 27, &autopilot->auto_fweight) < 0 ||
        btech_special_column_int(statement, 28, &autopilot->auto_nervous) < 0 ||
        btech_special_column_int(statement, 29, &autopilot->b_msc) < 0 ||
        btech_special_column_int(statement, 30, &autopilot->w_msc) < 0 ||
        btech_special_column_int(statement, 31, &autopilot->b_bsc) < 0 ||
        btech_special_column_int(statement, 32, &autopilot->w_bsc) < 0 ||
        btech_special_column_int(statement, 33, &autopilot->b_dan) < 0 ||
        btech_special_column_int(statement, 34, &autopilot->w_dan) < 0 ||
        btech_special_column_int(statement, 35, &autopilot->last_upd) < 0 ||
        (mech_dbref != 0 && !getMech(mech_dbref)) ||
        (map_dbref != NOTHING && map_dbref != 0 && !getMap(map_dbref))) {
      result = -1;
      break;
    }
    autopilot->mymechnum = mech_dbref;
    autopilot->mapindex = map_dbref;
    autopilot->target = target;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Resolve the live command callback from the durable command enum. */
static ACOM *btech_special_autopilot_command(int command_enum) {
  int index;

  for (index = 0; index < AUTO_NUM_COMMANDS && acom[index].name; index++)
    if (acom[index].command_enum == command_enum)
      return &acom[index];
  return NULL;
}

/* Load one command's ordered text arguments and derive its callback locally. */
static int btech_special_load_autopilot_command_args(
    sqlite3 *sqlite, AUTO *autopilot, DbRef autopilot_dbref, int position,
    int command_enum, int argument_count) {
  sqlite3_stmt *statement;
  ACOM *definition;
  command_node *command;
  DoublyLinkedListNode *list_node;
  const unsigned char *value;
  int argument_index;
  int length;
  int result;
  int step;

  definition = btech_special_autopilot_command(command_enum);
  if (!definition || argument_count != definition->argcount + 1 ||
      argument_count < 1 || argument_count > AUTOPILOT_MAX_ARGS)
    return -1;
  command = calloc(1, sizeof(*command));
  if (!command)
    return -1;
  statement = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT argument_index, value FROM btech_autopilot_command_args "
               "WHERE autopilot_dbref = ? AND command_position = ? "
               "ORDER BY argument_index;",
               -1, &statement, NULL) == SQLITE_OK &&
                   btech_special_bind_int(statement, 1, autopilot_dbref) == 0 &&
                   btech_special_bind_int(statement, 2, position) == 0
               ? 0
               : -1;
  argument_index = 0;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    int stored_index;

    value = sqlite3_column_text(statement, 1);
    length = sqlite3_column_bytes(statement, 1);
    if (btech_special_column_int(statement, 0, &stored_index) < 0 ||
        stored_index != argument_index || !value || length < 0 ||
        length >= LBUF_SIZE || (int)strlen((const char *)value) != length) {
      result = -1;
      break;
    }
    command->args[argument_index] =
        strndup((const char *)value, (size_t)length);
    if (!command->args[argument_index]) {
      result = -1;
      break;
    }
    argument_index++;
  }
  if (result == 0 && (step != SQLITE_DONE || argument_index != argument_count))
    result = -1;
  sqlite3_finalize(statement);
  if (result < 0) {
    auto_destroy_command_node(command);
    return -1;
  }
  command->argcount = (unsigned char)(argument_count - 1);
  command->command_enum = command_enum;
  command->ai_command_function = definition->ai_command_function;
  list_node = doubly_linked_list_create_node(command);
  if (!list_node) {
    auto_destroy_command_node(command);
    return -1;
  }
  doubly_linked_list_insert_end(autopilot->commands, list_node);
  return 0;
}

/* Restore the command queue in stable execution order. */
static int btech_special_load_autopilot_commands(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  AUTO *autopilot;
  DbRef current_autopilot;
  DbRef autopilot_dbref;
  int argument_count;
  int command_enum;
  int expected_position;
  int position;
  int result;
  int step;

  statement = NULL;
  current_autopilot = NOTHING;
  expected_position = 0;
  autopilot = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT autopilot_dbref, position, command_enum, arg_count "
          "FROM btech_autopilot_commands ORDER BY autopilot_dbref, position;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &autopilot_dbref) < 0 ||
        autopilot_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &position) < 0 ||
        btech_special_column_int(statement, 2, &command_enum) < 0 ||
        btech_special_column_int(statement, 3, &argument_count) < 0) {
      result = -1;
      break;
    }
    if (autopilot_dbref != current_autopilot) {
      autopilot = btech_special_object(autopilot_dbref, GTYPE_AUTO);
      if (!autopilot || !autopilot->commands ||
          doubly_linked_list_size(autopilot->commands)) {
        result = -1;
        break;
      }
      current_autopilot = autopilot_dbref;
      expected_position = 0;
    }
    if (position != expected_position ||
        btech_special_load_autopilot_command_args(
            sqlite, autopilot, autopilot_dbref, position, command_enum,
            argument_count) < 0) {
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

/* Restore an A* path as ordered nodes, never as saved list pointers. */
static int btech_special_load_autopilot_path(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  AUTO *autopilot;
  astar_node *path_node;
  DbRef current_autopilot;
  DbRef autopilot_dbref;
  DoublyLinkedListNode *list_node;
  long f_score;
  long g_score;
  long h_score;
  long hex_offset;
  int expected_position;
  int parent_x;
  int parent_y;
  int position;
  int result;
  int step;
  int x;
  int y;

  statement = NULL;
  current_autopilot = NOTHING;
  expected_position = 0;
  autopilot = NULL;
  result =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT autopilot_dbref, position, x, y, parent_x, parent_y, "
          "g_score, h_score, f_score, hex_offset FROM btech_autopilot_path "
          "ORDER BY autopilot_dbref, position;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &autopilot_dbref) < 0 ||
        autopilot_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &position) < 0 ||
        btech_special_column_int(statement, 2, &x) < 0 ||
        btech_special_column_int(statement, 3, &y) < 0 ||
        btech_special_column_int(statement, 4, &parent_x) < 0 ||
        btech_special_column_int(statement, 5, &parent_y) < 0 ||
        btech_special_column_long(statement, 6, &g_score) < 0 ||
        btech_special_column_long(statement, 7, &h_score) < 0 ||
        btech_special_column_long(statement, 8, &f_score) < 0 ||
        btech_special_column_long(statement, 9, &hex_offset) < 0 ||
        x < SHRT_MIN || x > SHRT_MAX || y < SHRT_MIN || y > SHRT_MAX ||
        parent_x < SHRT_MIN || parent_x > SHRT_MAX || parent_y < SHRT_MIN ||
        parent_y > SHRT_MAX) {
      result = -1;
      break;
    }
    if (autopilot_dbref != current_autopilot) {
      autopilot = btech_special_object(autopilot_dbref, GTYPE_AUTO);
      if (!autopilot || autopilot->astar_path) {
        result = -1;
        break;
      }
      autopilot->astar_path = doubly_linked_list_create_list();
      if (!autopilot->astar_path) {
        result = -1;
        break;
      }
      current_autopilot = autopilot_dbref;
      expected_position = 0;
    }
    if (position != expected_position) {
      result = -1;
      break;
    }
    path_node = calloc(1, sizeof(*path_node));
    if (!path_node) {
      result = -1;
      break;
    }
    path_node->x = (short)x;
    path_node->y = (short)y;
    path_node->x_parent = (short)parent_x;
    path_node->y_parent = (short)parent_y;
    path_node->g_score = g_score;
    path_node->h_score = h_score;
    path_node->f_score = f_score;
    path_node->hexoffset = hex_offset;
    list_node = doubly_linked_list_create_node(path_node);
    if (!list_node) {
      free(path_node);
      result = -1;
      break;
    }
    doubly_linked_list_insert_end(autopilot->astar_path, list_node);
    expected_position++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Store the bounded explicit state of repair consoles and dropship turrets. */
static int btech_store_simple_object(void *key, void *data, int depth,
                                     void *argument) {
  BTECH_OBJECT_STORE_CONTEXT *context = argument;
  XCODE *xcode = data;
  MECHREP *mechrep;
  TURRET_T *turret;
  AUTO *autopilot;
  MECH *mech;
  int index;
  int slot;
  int argument_index;
  int runtime_index;
  command_node *command;
  astar_node *path_node;
  damageNode *damage;

  (void)depth;
  if (context->result < 0)
    return 0;
  if (xcode->type == GTYPE_MECH) {
    mech = (MECH *)xcode;
    if (btech_special_bind_int(context->mech, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->mech, 2, mech->ID[0]) < 0 ||
        btech_special_bind_int(context->mech, 3, mech->ID[1]) < 0 ||
        btech_special_bind_int(context->mech, 4, mech->brief) < 0 ||
        btech_special_bind_int(context->mech, 5, mech->mapnumber) < 0 ||
        btech_special_bind_int(context->mech, 6, mech->mapindex) < 0 ||
        sqlite3_bind_text(context->mech, 7, mech->ud.mech_name, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 8, mech->ud.mech_type, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 9, mech->ud.unit_era, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 10, mech->ud.unit_tro, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        btech_special_bind_int(context->mech, 11, mech->ud.type) < 0 ||
        btech_special_bind_int(context->mech, 12, mech->ud.move) < 0 ||
        btech_special_bind_int(context->mech, 13, mech->ud.tac_range) < 0 ||
        btech_special_bind_int(context->mech, 14, mech->ud.lrs_range) < 0 ||
        btech_special_bind_int(context->mech, 15, mech->ud.scan_range) < 0 ||
        btech_special_bind_int(context->mech, 16, mech->ud.numsinks) < 0 ||
        btech_special_bind_int(context->mech, 17, mech->ud.hsengoverride) < 0 ||
        btech_special_bind_int(context->mech, 18, mech->ud.computer) < 0 ||
        btech_special_bind_int(context->mech, 19, mech->ud.radio) < 0 ||
        btech_special_bind_int(context->mech, 20, mech->ud.radioinfo) < 0 ||
        btech_special_bind_int(context->mech, 21, mech->ud.si) < 0 ||
        btech_special_bind_int(context->mech, 22, mech->ud.si_orig) < 0 ||
        btech_special_bind_int(context->mech, 23, mech->ud.radio_range) < 0 ||
        btech_special_bind_int(context->mech, 24, mech->ud.fuel) < 0 ||
        btech_special_bind_int(context->mech, 25, mech->ud.fuel_orig) < 0 ||
        btech_special_bind_int(context->mech, 26, mech->ud.tons) < 0 ||
        btech_special_bind_int(context->mech, 27, mech->ud.walkspeed) < 0 ||
        btech_special_bind_int(context->mech, 28, mech->ud.runspeed) < 0 ||
        sqlite3_bind_double(context->mech, 29, mech->ud.maxspeed) !=
            SQLITE_OK ||
        sqlite3_bind_double(context->mech, 30, mech->ud.template_maxspeed) !=
            SQLITE_OK ||
        btech_special_bind_int(context->mech, 31, mech->ud.mechbv) < 0 ||
        btech_special_bind_int(context->mech, 32, mech->ud.cargospace) < 0 ||
        btech_special_bind_int(context->mech, 33, mech->ud.targcomp) < 0 ||
        btech_special_bind_int(context->mech, 34, mech->ud.carmaxton) < 0 ||
        btech_special_step(context->mech) < 0)
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_SECTIONS; index++) {
      struct section_struct *section = &mech->ud.sections[index];
      if (btech_special_bind_int(context->section, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->section, 2, index) < 0 ||
          btech_special_bind_int(context->section, 3, section->armor) < 0 ||
          btech_special_bind_int(context->section, 4, section->internal) < 0 ||
          btech_special_bind_int(context->section, 5, section->rear) < 0 ||
          btech_special_bind_int(context->section, 6, section->armor_orig) <
              0 ||
          btech_special_bind_int(context->section, 7, section->internal_orig) <
              0 ||
          btech_special_bind_int(context->section, 8, section->rear_orig) < 0 ||
          btech_special_bind_int(context->section, 9, section->basetohit) < 0 ||
          btech_special_bind_int(context->section, 10, section->config) < 0 ||
          btech_special_bind_int(context->section, 11, section->recycle) < 0 ||
          btech_special_bind_int(context->section, 12, section->specials) < 0 ||
          btech_special_step(context->section) < 0) {
        context->result = -1;
        break;
      }
      for (slot = 0; context->result == 0 && slot < NUM_CRITICALS; slot++) {
        struct critical_slot *critical = &section->criticals[slot];
        if (btech_special_bind_int(context->critical, 1, (DbRef)key) < 0 ||
            btech_special_bind_int(context->critical, 2, index) < 0 ||
            btech_special_bind_int(context->critical, 3, slot) < 0 ||
            btech_special_bind_int(context->critical, 4, critical->brand) < 0 ||
            btech_special_bind_int(context->critical, 5, critical->data) < 0 ||
            btech_special_bind_int(context->critical, 6, critical->type) < 0 ||
            btech_special_bind_int(context->critical, 7, critical->firemode) <
                0 ||
            btech_special_bind_int(context->critical, 8, critical->ammomode) <
                0 ||
            btech_special_bind_int(context->critical, 9,
                                   critical->weapDamageFlags) < 0 ||
            btech_special_bind_int(context->critical, 10,
                                   critical->desiredAmmoLoc) < 0 ||
            btech_special_step(context->critical) < 0)
          context->result = -1;
      }
    }
    if (context->result == 0 &&
        (btech_special_bind_int(context->position, 1, (DbRef)key) < 0 ||
         btech_special_bind_int(context->position, 2, mech->pd.pilotstatus) <
             0 ||
         btech_special_bind_int(context->position, 3, mech->pd.terrain) < 0 ||
         btech_special_bind_int(context->position, 4, mech->pd.elev) < 0 ||
         sqlite3_bind_double(context->position, 5, mech->pd.hexes_walked) !=
             SQLITE_OK ||
         btech_special_bind_int(context->position, 6, mech->pd.facing) < 0 ||
         btech_special_bind_int(context->position, 7, mech->pd.x) < 0 ||
         btech_special_bind_int(context->position, 8, mech->pd.y) < 0 ||
         btech_special_bind_int(context->position, 9, mech->pd.z) < 0 ||
         btech_special_bind_int(context->position, 10, mech->pd.last_x) < 0 ||
         btech_special_bind_int(context->position, 11, mech->pd.last_y) < 0 ||
         sqlite3_bind_double(context->position, 12, mech->pd.fx) != SQLITE_OK ||
         sqlite3_bind_double(context->position, 13, mech->pd.fy) != SQLITE_OK ||
         sqlite3_bind_double(context->position, 14, mech->pd.fz) != SQLITE_OK ||
         btech_special_bind_int(context->position, 15, mech->pd.team) < 0 ||
         btech_special_bind_int(context->position, 16, mech->pd.unusable_arcs) <
             0 ||
         btech_special_bind_int(context->position, 17, mech->pd.stall) < 0 ||
         btech_special_bind_int(context->position, 18, mech->pd.pilot) < 0 ||
         btech_special_step(context->position) < 0))
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_BAYS; index++) {
      if (btech_special_bind_int(context->bay, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->bay, 2, index) < 0 ||
          btech_special_bind_int(context->bay, 3, mech->pd.bay[index]) < 0 ||
          btech_special_step(context->bay) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < NUM_TURRETS; index++) {
      if (btech_special_bind_int(context->mech_turret, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->mech_turret, 2, index) < 0 ||
          btech_special_bind_int(context->mech_turret, 3,
                                 mech->pd.turret[index]) < 0 ||
          btech_special_step(context->mech_turret) < 0)
        context->result = -1;
    }
    if (context->result == 0 &&
        (btech_special_bind_int(context->c3, 1, (DbRef)key) < 0 ||
         sqlite3_bind_text(context->c3, 2, mech->sd.C3ChanTitle, -1,
                           SQLITE_TRANSIENT) != SQLITE_OK ||
         btech_special_bind_int(context->c3, 3, mech->sd.wC3iNetworkSize) < 0 ||
         btech_special_bind_int(context->c3, 4, mech->sd.wC3NetworkSize) < 0 ||
         btech_special_bind_int(context->c3, 5, mech->sd.wTotalC3Masters) < 0 ||
         btech_special_bind_int(context->c3, 6, mech->sd.wWorkingC3Masters) <
             0 ||
         btech_special_bind_int(context->c3, 7, mech->sd.C3FreqMode) < 0 ||
         btech_special_bind_int(context->c3, 8, mech->sd.tagTarget) < 0 ||
         btech_special_bind_int(context->c3, 9, mech->sd.taggedBy) < 0 ||
         btech_special_step(context->c3) < 0))
      context->result = -1;
    for (index = 0;
         context->result == 0 && index < C3I_NETWORK_SIZE + C3_NETWORK_SIZE;
         index++) {
      DbRef node = index < C3I_NETWORK_SIZE
                       ? mech->sd.C3iNetwork[index]
                       : mech->sd.C3Network[index - C3I_NETWORK_SIZE];
      int network = index < C3I_NETWORK_SIZE ? 0 : 1;
      int node_index =
          index < C3I_NETWORK_SIZE ? index : index - C3I_NETWORK_SIZE;
      if (btech_special_bind_int(context->c3node, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->c3node, 2, network) < 0 ||
          btech_special_bind_int(context->c3node, 3, node_index) < 0 ||
          btech_special_bind_int(context->c3node, 4, node) < 0 ||
          btech_special_step(context->c3node) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < NUM_TICS; index++) {
      for (slot = 0; context->result == 0 && slot < TICLONGS; slot++) {
        if (btech_special_bind_int(context->tic, 1, (DbRef)key) < 0 ||
            btech_special_bind_int(context->tic, 2, index) < 0 ||
            btech_special_bind_int(context->tic, 3, slot) < 0 ||
            btech_special_bind_int(context->tic, 4, mech->tic[index][slot]) <
                0 ||
            btech_special_step(context->tic) < 0)
          context->result = -1;
      }
    }
    for (index = 0; context->result == 0 && index < FREQS; index++) {
      if (btech_special_bind_int(context->frequency, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->frequency, 2, index) < 0 ||
          btech_special_bind_int(context->frequency, 3, mech->freq[index]) <
              0 ||
          btech_special_bind_int(context->frequency, 4,
                                 mech->freqmodes[index]) < 0 ||
          sqlite3_bind_text(context->frequency, 5, mech->chantitle[index], -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          btech_special_step(context->frequency) < 0)
        context->result = -1;
    }
    if (context->result == 0) {
      runtime_index = 1;
#define BTECH_RUNTIME_INT(value)                                               \
  btech_special_bind_int(context->runtime, runtime_index++,                    \
                         (sqlite3_int64)(value))
#define BTECH_RUNTIME_REAL(value)                                              \
  btech_special_bind_real(context->runtime, runtime_index++, (double)(value))
      if (BTECH_RUNTIME_INT((DbRef)key) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.jumptop) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.aim) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.basetohit) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.pilotskillbase) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.engineheat) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.masc_value) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.aim_type) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.sensor[0]) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.sensor[1]) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.fire_adjustment) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.vis_mod) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.chargetimer) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.chargedist) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.staggerstamp) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.mech_prefs) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.jumplength) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.goingx) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.goingy) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.desiredfacing) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.angle) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.jumpheading) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.targx) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.targy) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.targz) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.turretfacing) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.turndamage) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lateral) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.num_seen) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lx) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.ly) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.chgtarget) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.dfatarget) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.target) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.swarming) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.swarmedby) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.carrying) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.spotter) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.heat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.weapheat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.plus_heat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.minus_heat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.startfx) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.startfy) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.startfz) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.endfz) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.verticalspeed) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.speed) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.desired_speed) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.jumpspeed) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.critstatus) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.status) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.status2) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.specials) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.specials2) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.specialsstatus) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.tankcritstatus) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.last_weapon_recycle) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.cargo_weight) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastrndu) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.rnd) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.last_ds_msg) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.boom_start) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.maxfuel) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastused) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.cocoon) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.commconv) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.commconv_last) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.onumsinks) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.disabled_hs) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.autopilot_num) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.heatboom_last) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.sspin) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.can_see) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.row) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.rcw) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.rspd) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.erat) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.per) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.wxf) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.last_startup) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.maxsuits) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.infantry_specials) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.scharge_value) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.staggerDamage) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastStaggerNotify) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.critstatus2) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.xpmod) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.shots_fired) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.shots_hit) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.shots_missed) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.damage_taken) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.damage_inflicted) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.units_killed) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastStaggerCheck) < 0 ||
          btech_special_step(context->runtime) < 0)
        context->result = -1;
#undef BTECH_RUNTIME_INT
#undef BTECH_RUNTIME_REAL
    }
    for (index = 0; context->result == 0 && index < 5; index++) {
      if (btech_special_bind_int(context->runtime_unused, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->runtime_unused, 2, index) < 0 ||
          btech_special_bind_int(context->runtime_unused, 3,
                                 mech->rd.unused[index]) < 0 ||
          btech_special_step(context->runtime_unused) < 0)
        context->result = -1;
    }
#ifndef BT_CALCULATE_BV
    for (index = 0; context->result == 0 && index < 8; index++) {
      if (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->unit_aux, 2, index) < 0 ||
          btech_special_bind_int(context->unit_aux, 3, mech->ud.unused[index]) <
              0 ||
          btech_special_step(context->unit_aux) < 0)
        context->result = -1;
    }
#else
    if (context->result == 0 &&
        (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
         btech_special_bind_int(context->unit_aux, 2, 0) < 0 ||
         btech_special_bind_int(context->unit_aux, 3, mech->ud.mechbv_last) <
             0 ||
         btech_special_step(context->unit_aux) < 0))
      context->result = -1;
#endif
    for (index = 0; context->result == 0 && index < 3; index++) {
      if (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->unit_aux, 2, 8 + index) < 0 ||
          btech_special_bind_int(context->unit_aux, 3,
                                 mech->ud.unused_char[index]) < 0 ||
          btech_special_step(context->unit_aux) < 0)
        context->result = -1;
    }
    for (damage = mech->rd.staggerDamageList, index = 0;
         context->result == 0 && damage; damage = damage->next, index++) {
      if (btech_special_bind_int(context->stagger_damage, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->stagger_damage, 2, index) < 0 ||
          btech_special_bind_int(context->stagger_damage, 3, damage->amount) <
              0 ||
          btech_special_bind_int(context->stagger_damage, 4,
                                 (sqlite3_int64)damage->occuredAt) < 0 ||
          btech_special_bind_int(context->stagger_damage, 5,
                                 damage->attackerNum) < 0 ||
          btech_special_bind_int(context->stagger_damage, 6, damage->counted) <
              0 ||
          btech_special_step(context->stagger_damage) < 0)
        context->result = -1;
    }
  } else if (xcode->type == GTYPE_MECHREP) {
    mechrep = (MECHREP *)xcode;
    if (btech_special_bind_int(context->mechrep, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->mechrep, 2, mechrep->current_target) <
            0 ||
        btech_special_step(context->mechrep) < 0)
      context->result = -1;
  } else if (xcode->type == GTYPE_TURRET) {
    turret = (TURRET_T *)xcode;
    if (btech_special_bind_int(context->turret, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->turret, 2, turret->arcs) < 0 ||
        btech_special_bind_int(context->turret, 3, turret->parent) < 0 ||
        btech_special_bind_int(context->turret, 4, turret->gunner) < 0 ||
        btech_special_bind_int(context->turret, 5, turret->target) < 0 ||
        btech_special_bind_int(context->turret, 6, turret->targx) < 0 ||
        btech_special_bind_int(context->turret, 7, turret->targy) < 0 ||
        btech_special_bind_int(context->turret, 8, turret->targz) < 0 ||
        btech_special_bind_int(context->turret, 9, turret->lockmode) < 0 ||
        btech_special_step(context->turret) < 0)
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_TICS; index++) {
      if (btech_special_bind_int(context->turret_tic, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->turret_tic, 2, index) < 0 ||
          btech_special_bind_int(context->turret_tic, 3, turret->tic[index]) <
              0 ||
          btech_special_step(context->turret_tic) < 0)
        context->result = -1;
    }
  } else if (xcode->type == GTYPE_AUTO) {
    autopilot = (AUTO *)xcode;
    if (btech_special_bind_int(context->autopilot, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->autopilot, 2, autopilot->mymechnum) <
            0 ||
        btech_special_bind_int(context->autopilot, 3, autopilot->mapindex) <
            0 ||
        btech_special_bind_int(context->autopilot, 4, autopilot->speed) < 0 ||
        btech_special_bind_int(context->autopilot, 5, autopilot->ofsx) < 0 ||
        btech_special_bind_int(context->autopilot, 6, autopilot->ofsy) < 0 ||
        btech_special_bind_int(context->autopilot, 7,
                               autopilot->verbose_level) < 0 ||
        btech_special_bind_int(context->autopilot, 8, autopilot->target) < 0 ||
        btech_special_bind_int(context->autopilot, 9, autopilot->target_score) <
            0 ||
        btech_special_bind_int(context->autopilot, 10,
                               autopilot->target_threshold) < 0 ||
        btech_special_bind_int(context->autopilot, 11,
                               autopilot->target_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 12,
                               autopilot->chase_target) < 0 ||
        btech_special_bind_int(context->autopilot, 13,
                               autopilot->chasetarg_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 14,
                               autopilot->follow_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 15, autopilot->flags) < 0 ||
        btech_special_bind_int(context->autopilot, 16,
                               autopilot->mech_max_range) < 0 ||
        btech_special_bind_int(context->autopilot, 17, autopilot->roam_type) <
            0 ||
        btech_special_bind_int(context->autopilot, 18,
                               autopilot->roam_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 19,
                               autopilot->roam_target_hex_x) < 0 ||
        btech_special_bind_int(context->autopilot, 20,
                               autopilot->roam_target_hex_y) < 0 ||
        btech_special_bind_int(context->autopilot, 21,
                               autopilot->roam_anchor_hex_x) < 0 ||
        btech_special_bind_int(context->autopilot, 22,
                               autopilot->roam_anchor_hex_y) < 0 ||
        btech_special_bind_int(context->autopilot, 23,
                               autopilot->roam_anchor_distance) < 0 ||
        btech_special_bind_int(context->autopilot, 24, autopilot->ahead_ok) <
            0 ||
        btech_special_bind_int(context->autopilot, 25, autopilot->auto_cmode) <
            0 ||
        btech_special_bind_int(context->autopilot, 26, autopilot->auto_cdist) <
            0 ||
        btech_special_bind_int(context->autopilot, 27,
                               autopilot->auto_goweight) < 0 ||
        btech_special_bind_int(context->autopilot, 28,
                               autopilot->auto_fweight) < 0 ||
        btech_special_bind_int(context->autopilot, 29,
                               autopilot->auto_nervous) < 0 ||
        btech_special_bind_int(context->autopilot, 30, autopilot->b_msc) < 0 ||
        btech_special_bind_int(context->autopilot, 31, autopilot->w_msc) < 0 ||
        btech_special_bind_int(context->autopilot, 32, autopilot->b_bsc) < 0 ||
        btech_special_bind_int(context->autopilot, 33, autopilot->w_bsc) < 0 ||
        btech_special_bind_int(context->autopilot, 34, autopilot->b_dan) < 0 ||
        btech_special_bind_int(context->autopilot, 35, autopilot->w_dan) < 0 ||
        btech_special_bind_int(context->autopilot, 36, autopilot->last_upd) <
            0 ||
        btech_special_step(context->autopilot) < 0)
      context->result = -1;
    for (index = 1; context->result == 0 && autopilot->commands &&
                    index <= doubly_linked_list_size(autopilot->commands);
         index++) {
      command = (command_node *)doubly_linked_list_get_node(autopilot->commands,
                                                            index);
      if (!command || command->argcount >= AUTOPILOT_MAX_ARGS ||
          btech_special_bind_int(context->autopilot_command, 1, (DbRef)key) <
              0 ||
          btech_special_bind_int(context->autopilot_command, 2, index - 1) <
              0 ||
          btech_special_bind_int(context->autopilot_command, 3,
                                 command->command_enum) < 0 ||
          btech_special_bind_int(context->autopilot_command, 4,
                                 command->argcount + 1) < 0 ||
          btech_special_step(context->autopilot_command) < 0) {
        context->result = -1;
        break;
      }
      for (argument_index = 0;
           context->result == 0 && argument_index <= command->argcount;
           argument_index++) {
        if (!command->args[argument_index] ||
            btech_special_bind_int(context->autopilot_command_arg, 1,
                                   (DbRef)key) < 0 ||
            btech_special_bind_int(context->autopilot_command_arg, 2,
                                   index - 1) < 0 ||
            btech_special_bind_int(context->autopilot_command_arg, 3,
                                   argument_index) < 0 ||
            sqlite3_bind_text(context->autopilot_command_arg, 4,
                              command->args[argument_index], -1,
                              SQLITE_TRANSIENT) != SQLITE_OK ||
            btech_special_step(context->autopilot_command_arg) < 0)
          context->result = -1;
      }
    }
    for (index = 1; context->result == 0 && autopilot->astar_path &&
                    index <= doubly_linked_list_size(autopilot->astar_path);
         index++) {
      path_node = (astar_node *)doubly_linked_list_get_node(
          autopilot->astar_path, index);
      if (!path_node ||
          btech_special_bind_int(context->autopilot_path, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->autopilot_path, 2, index - 1) < 0 ||
          btech_special_bind_int(context->autopilot_path, 3, path_node->x) <
              0 ||
          btech_special_bind_int(context->autopilot_path, 4, path_node->y) <
              0 ||
          btech_special_bind_int(context->autopilot_path, 5,
                                 path_node->x_parent) < 0 ||
          btech_special_bind_int(context->autopilot_path, 6,
                                 path_node->y_parent) < 0 ||
          btech_special_bind_int(context->autopilot_path, 7,
                                 path_node->g_score) < 0 ||
          btech_special_bind_int(context->autopilot_path, 8,
                                 path_node->h_score) < 0 ||
          btech_special_bind_int(context->autopilot_path, 9,
                                 path_node->f_score) < 0 ||
          btech_special_bind_int(context->autopilot_path, 10,
                                 path_node->hexoffset) < 0 ||
          btech_special_step(context->autopilot_path) < 0) {
        context->result = -1;
        break;
      }
    }
  }
  return context->result == 0;
}

/* Store one map's scalar state plus its explicit occupancy and LOS matrices. */
static int btech_store_map(void *key, void *data, int depth, void *argument) {
  BTECH_MAP_STORE_CONTEXT *context = argument;
  XCODE *xcode = data;
  MAP *map;
  int index;
  int target;
  int object_type;
  int ordinal;
  int byte_index;
  int bytes_per_row;
  int x;
  int y;
  mapobj *object;
  unsigned char **bits;

  (void)depth;
  if (context->result < 0 || xcode->type != GTYPE_MAP)
    return context->result == 0;
  map = (MAP *)xcode;
  if (btech_special_bind_int(context->map, 1, (DbRef)key) < 0 ||
      sqlite3_bind_text(context->map, 2, map->mapname, -1, SQLITE_TRANSIENT) !=
          SQLITE_OK ||
      btech_special_bind_int(context->map, 3, map->map_width) < 0 ||
      btech_special_bind_int(context->map, 4, map->map_height) < 0 ||
      btech_special_bind_int(context->map, 5, map->temp) < 0 ||
      btech_special_bind_int(context->map, 6, map->grav) < 0 ||
      btech_special_bind_int(context->map, 7, map->cloudbase) < 0 ||
      btech_special_bind_int(context->map, 8, map->mapvis) < 0 ||
      btech_special_bind_int(context->map, 9, map->maxvis) < 0 ||
      btech_special_bind_int(context->map, 10, map->maplight) < 0 ||
      btech_special_bind_int(context->map, 11, map->winddir) < 0 ||
      btech_special_bind_int(context->map, 12, map->windspeed) < 0 ||
      btech_special_bind_int(context->map, 13, map->unused_char) < 0 ||
      btech_special_bind_int(context->map, 14, map->flags) < 0 ||
      btech_special_bind_int(context->map, 15, map->cf) < 0 ||
      btech_special_bind_int(context->map, 16, map->cfmax) < 0 ||
      btech_special_bind_int(context->map, 17, map->onmap) < 0 ||
      btech_special_bind_int(context->map, 18, map->buildflag) < 0 ||
      btech_special_bind_int(context->map, 19, map->first_free) < 0 ||
      btech_special_bind_int(context->map, 20, map->moves) < 0 ||
      btech_special_bind_int(context->map, 21, map->movemod) < 0 ||
      btech_special_bind_int(context->map, 22, map->sensorflags) < 0 ||
      btech_special_bind_int(context->map, 23, map->regen_factor) < 0 ||
      btech_special_step(context->map) < 0) {
    context->result = -1;
    return 0;
  }
  if (!map->map) {
    context->result = -1;
    return 0;
  }
  for (y = 0; context->result == 0 && y < map->map_height; y++) {
    for (x = 0; context->result == 0 && x < map->map_width; x++) {
      if (btech_special_bind_int(context->hex, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->hex, 2, x) < 0 ||
          btech_special_bind_int(context->hex, 3, y) < 0 ||
          btech_special_bind_int(context->hex, 4, map->map[y][x]) < 0 ||
          btech_special_step(context->hex) < 0)
        context->result = -1;
    }
  }
  for (index = 0; context->result == 0 && index < map->first_free; index++) {
    if (btech_special_bind_int(context->slot, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->slot, 2, index) < 0 ||
        btech_special_bind_int(context->slot, 3, map->mechsOnMap[index]) < 0 ||
        btech_special_bind_int(context->slot, 4, map->mechflags[index]) < 0 ||
        btech_special_step(context->slot) < 0) {
      context->result = -1;
      break;
    }
    for (target = 0; context->result == 0 && target < map->first_free;
         target++) {
      if (btech_special_bind_int(context->los, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->los, 2, index) < 0 ||
          btech_special_bind_int(context->los, 3, target) < 0 ||
          btech_special_bind_int(context->los, 4, map->LOSinfo[index][target]) <
              0 ||
          btech_special_step(context->los) < 0)
        context->result = -1;
    }
  }
  for (object_type = 0; context->result == 0 && object_type < NUM_MAPOBJTYPES;
       object_type++) {
    if (object_type == TYPE_BITS)
      continue;
    ordinal = 0;
    for (object = map->mapobj[object_type]; context->result == 0 && object;
         object = object->next, ordinal++) {
      if (btech_special_bind_int(context->object, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->object, 2, object_type) < 0 ||
          btech_special_bind_int(context->object, 3, ordinal) < 0 ||
          btech_special_bind_int(context->object, 4, object->x) < 0 ||
          btech_special_bind_int(context->object, 5, object->y) < 0 ||
          btech_special_bind_int(context->object, 6, object->obj) < 0 ||
          btech_special_bind_int(context->object, 7, object->datac) < 0 ||
          btech_special_bind_int(context->object, 8, object->datas) < 0 ||
          btech_special_bind_int(context->object, 9, object->datai) < 0 ||
          btech_special_step(context->object) < 0)
        context->result = -1;
    }
  }
  if (context->result == 0 && map->mapobj[TYPE_BITS]) {
    bits = (unsigned char **)(void *)map->mapobj[TYPE_BITS]->datai;
    bytes_per_row = map->map_width / 4 + (map->map_width % 4 ? 1 : 0);
    for (index = 0; context->result == 0 && index < map->map_height; index++) {
      if (!bits[index])
        continue;
      for (byte_index = 0; byte_index < bytes_per_row; byte_index++) {
        if (btech_special_bind_int(context->bits, 1, (DbRef)key) < 0 ||
            btech_special_bind_int(context->bits, 2, index) < 0 ||
            btech_special_bind_int(context->bits, 3, byte_index) < 0 ||
            btech_special_bind_int(context->bits, 4, bits[index][byte_index]) <
                0 ||
            btech_special_step(context->bits) < 0)
          context->result = -1;
      }
    }
  }
  return context->result == 0;
}

/* Capture one queued repair event in its durable SQLite representation. */
static sqlite3_stmt *btech_repair_statement;
static int btech_repair_type;
static int btech_repair_result;

static void btech_store_repair_event(MuxEvent *event) {
  MECH *mech = event->data;
  long remaining = event->tick - mux_event_tick;

  if (btech_repair_result < 0 || !mech)
    return;
  if (remaining < 1)
    remaining = 1;
  if (event->function == very_fake_func)
    remaining = -remaining;
  if (btech_special_bind_int(btech_repair_statement, 1, mech->mynum) < 0 ||
      btech_special_bind_int(btech_repair_statement, 2, btech_repair_type) <
          0 ||
      btech_special_bind_int(btech_repair_statement, 3,
                             remaining < 0 ? -remaining : remaining) < 0 ||
      btech_special_bind_int(btech_repair_statement, 4, (long)event->data2) <
          0 ||
      btech_special_bind_int(btech_repair_statement, 5, remaining < 0) < 0 ||
      btech_special_step(btech_repair_statement) < 0)
    btech_repair_result = -1;
}

/* Mirror map dynamic state and repair queues without changing legacy reads. */
static int btech_persistence_store_special_state(sqlite3 *sqlite) {
  BTECH_MAP_STORE_CONTEXT maps = {NULL, NULL, NULL, NULL, NULL, NULL, -1};
  BTECH_OBJECT_STORE_CONTEXT objects;
  sqlite3_stmt *repairs = NULL;
  int type;
  int result;

  btech_special_test_reset_fault();
  memset(&objects, 0, sizeof(objects));
  objects.result = -1;
  if (btech_special_exec(sqlite, btech_special_schema_sql) < 0)
    return -1;
  if (btech_special_store_metadata(sqlite) < 0)
    return -1;
  if (!xcode_tree)
    return 0;
  if (sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_maps VALUES (?, ?, ?, ?, ?, ?, ?, "
                         "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                         -1, &maps.map, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_map_hexes VALUES (?, ?, ?, ?);", -1,
                         &maps.hex, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_map_slots VALUES (?, ?, ?, ?);", -1,
                         &maps.slot, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_map_los VALUES (?, ?, ?, ?);", -1,
                         &maps.los, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_map_objects VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &maps.object, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_map_bits VALUES (?, ?, ?, ?);", -1,
                         &maps.bits, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (?, ?, ?, ?, ?);",
          -1, &repairs, NULL) != SQLITE_OK) {
    sqlite3_finalize(maps.map);
    sqlite3_finalize(maps.hex);
    sqlite3_finalize(maps.slot);
    sqlite3_finalize(maps.los);
    sqlite3_finalize(maps.object);
    sqlite3_finalize(maps.bits);
    sqlite3_finalize(repairs);
    return -1;
  }
  if (sqlite3_prepare_v2(sqlite, "INSERT INTO btech_mechrep VALUES (?, ?);", -1,
                         &objects.mechrep, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_turrets VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);", -1,
          &objects.turret, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_turret_tics VALUES (?, ?, ?);", -1,
                         &objects.turret_tic, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_autopilots VALUES (?, ?, ?, ?, ?, "
                         "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
                         "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                         -1, &objects.autopilot, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_mechs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &objects.mech, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_sections VALUES (?, ?, ?, ?, "
                         "?, ?, ?, ?, ?, ?, ?, ?);",
                         -1, &objects.section, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_criticals VALUES (?, ?, ?, ?, "
                         "?, ?, ?, ?, ?, ?);",
                         -1, &objects.critical, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_positions VALUES (?, ?, ?, ?, "
                         "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
                         -1, &objects.position, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_bays VALUES (?, ?, ?);", -1,
                         &objects.bay, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_turrets VALUES (?, ?, ?);", -1,
                         &objects.mech_turret, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_mech_c3 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);", -1,
          &objects.c3, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_c3_nodes VALUES (?, ?, ?, ?);",
                         -1, &objects.c3node, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_tics VALUES (?, ?, ?, ?);", -1,
                         &objects.tic, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite, "INSERT INTO btech_mech_frequencies VALUES (?, ?, ?, ?, ?);",
          -1, &objects.frequency, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_mech_runtime VALUES ("
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &objects.runtime, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite, "INSERT INTO btech_mech_runtime_unused VALUES (?, ?, ?);", -1,
          &objects.runtime_unused, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_mech_unit_aux VALUES (?, ?, ?);",
                         -1, &objects.unit_aux, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_mech_stagger_damage VALUES (?, ?, ?, ?, ?, ?);",
          -1, &objects.stagger_damage, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite, "INSERT INTO btech_autopilot_commands VALUES (?, ?, ?, ?);",
          -1, &objects.autopilot_command, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_autopilot_command_args VALUES (?, ?, ?, ?);", -1,
          &objects.autopilot_command_arg, NULL) != SQLITE_OK ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_autopilot_path VALUES (?, ?, ?, ?, "
                         "?, ?, ?, ?, ?, ?);",
                         -1, &objects.autopilot_path, NULL) != SQLITE_OK) {
    sqlite3_finalize(maps.map);
    sqlite3_finalize(maps.hex);
    sqlite3_finalize(maps.slot);
    sqlite3_finalize(maps.los);
    sqlite3_finalize(maps.object);
    sqlite3_finalize(maps.bits);
    sqlite3_finalize(repairs);
    btech_finalize_object_statements(&objects);
    return -1;
  }
  maps.result = 0;
  red_black_tree_walk(xcode_tree, WALK_INORDER, btech_store_map, &maps);
  objects.result = 0;
  red_black_tree_walk(xcode_tree, WALK_INORDER, btech_store_simple_object,
                      &objects);
  btech_repair_statement = repairs;
  btech_repair_result = 0;
  for (type = FIRST_TECH_EVENT;
       type <= LAST_TECH_EVENT && btech_repair_result == 0; type++) {
    btech_repair_type = type;
    mux_event_gothru_type(type, btech_store_repair_event);
  }
  result =
      maps.result < 0 || objects.result < 0 || btech_repair_result < 0 ? -1 : 0;
  sqlite3_finalize(maps.map);
  sqlite3_finalize(maps.hex);
  sqlite3_finalize(maps.slot);
  sqlite3_finalize(maps.los);
  sqlite3_finalize(maps.object);
  sqlite3_finalize(maps.bits);
  sqlite3_finalize(repairs);
  btech_finalize_object_statements(&objects);
  return result;
}

/* Reads remain on the legacy files during this first BTech dual-write slice. */
static int btech_persistence_preload_special_state(sqlite3 *sqlite) {
  (void)sqlite;
  return 0;
}

typedef struct btech_special_object_counts BTECH_SPECIAL_OBJECT_COUNTS;
struct btech_special_object_counts {
  int maps;
  int mechs;
  int mechreps;
  int turrets;
  int autopilots;
};

/* Count normal BTech instances, excluding DEBUG and other non-persisted types.
 */
static int btech_special_count_objects(void *key, void *data, int depth,
                                       void *argument) {
  BTECH_SPECIAL_OBJECT_COUNTS *counts = argument;
  XCODE *xcode = data;

  (void)key;
  (void)depth;
  switch (xcode->type) {
  case GTYPE_MAP:
    counts->maps++;
    break;
  case GTYPE_MECH:
    counts->mechs++;
    break;
  case GTYPE_MECHREP:
    counts->mechreps++;
    break;
  case GTYPE_TURRET:
    counts->turrets++;
    break;
  case GTYPE_AUTO:
    counts->autopilots++;
    break;
  default:
    break;
  }
  return 1;
}

/* Read a count from one known table name in the fixed BTech schema. */
static int btech_special_table_count(sqlite3 *sqlite, const char *table,
                                     int *count) {
  sqlite3_stmt *statement;
  char sql[128];
  int result;

  statement = NULL;
  if (snprintf(sql, sizeof(sql), "SELECT count(*) FROM %s;", table) < 0)
    return -1;
  result = sqlite3_prepare_v2(sqlite, sql, -1, &statement, NULL) == SQLITE_OK &&
                   sqlite3_step(statement) == SQLITE_ROW &&
                   btech_special_column_int(statement, 0, count) == 0 &&
                   sqlite3_step(statement) == SQLITE_DONE
               ? 0
               : -1;
  sqlite3_finalize(statement);
  return result;
}

/* Require one parent and every fixed child row for each preallocated object. */
static int btech_special_validate_required_rows(sqlite3 *sqlite) {
  BTECH_SPECIAL_OBJECT_COUNTS counts = {0, 0, 0, 0, 0};
  int expected;
  int actual;

  red_black_tree_walk(xcode_tree, WALK_INORDER, btech_special_count_objects,
                      &counts);
#define REQUIRE_ROWS(table, rows)                                              \
  do {                                                                         \
    expected = (rows);                                                         \
    if (expected < 0 ||                                                        \
        btech_special_table_count(sqlite, table, &actual) < 0 ||               \
        actual != expected)                                                    \
      return -1;                                                               \
  } while (0)
  REQUIRE_ROWS("btech_maps", counts.maps);
  REQUIRE_ROWS("btech_mechs", counts.mechs);
  REQUIRE_ROWS("btech_mechrep", counts.mechreps);
  REQUIRE_ROWS("btech_turrets", counts.turrets);
  REQUIRE_ROWS("btech_autopilots", counts.autopilots);
  REQUIRE_ROWS("btech_mech_sections", counts.mechs * NUM_SECTIONS);
  REQUIRE_ROWS("btech_mech_criticals",
               counts.mechs * NUM_SECTIONS * NUM_CRITICALS);
  REQUIRE_ROWS("btech_mech_positions", counts.mechs);
  REQUIRE_ROWS("btech_mech_bays", counts.mechs * NUM_BAYS);
  REQUIRE_ROWS("btech_mech_turrets", counts.mechs * NUM_TURRETS);
  REQUIRE_ROWS("btech_mech_c3", counts.mechs);
  REQUIRE_ROWS("btech_mech_c3_nodes",
               counts.mechs * (C3I_NETWORK_SIZE + C3_NETWORK_SIZE));
  REQUIRE_ROWS("btech_mech_tics", counts.mechs * NUM_TICS * TICLONGS);
  REQUIRE_ROWS("btech_mech_frequencies", counts.mechs * FREQS);
  REQUIRE_ROWS("btech_mech_runtime", counts.mechs);
  REQUIRE_ROWS("btech_mech_runtime_unused", counts.mechs * 5);
#ifndef BT_CALCULATE_BV
  REQUIRE_ROWS("btech_mech_unit_aux", counts.mechs * 11);
#else
  REQUIRE_ROWS("btech_mech_unit_aux", counts.mechs * 4);
#endif
  REQUIRE_ROWS("btech_turret_tics", counts.turrets * NUM_TICS);
#undef REQUIRE_ROWS
  return 0;
}

/* Load every BTech table only after the normal special-object allocators run.
 */
static int btech_special_load_all(sqlite3 *sqlite) {
#define BTECH_LOAD(stage, function)                                            \
  do {                                                                         \
    if ((function)(sqlite) < 0) {                                              \
      log_error(LOG_ALWAYS, "BTP", "FAIL",                                     \
                "SQLite BTech validation failed at %s.", (char *)stage);       \
      return -1;                                                               \
    }                                                                          \
  } while (0)
  BTECH_LOAD("metadata", btech_special_validate_metadata);
  BTECH_LOAD("required rows", btech_special_validate_required_rows);
  BTECH_LOAD("map parents", btech_special_load_map_parents);
  BTECH_LOAD("map hexes", btech_special_load_map_hexes);
  BTECH_LOAD("map slots", btech_special_load_map_slots);
  BTECH_LOAD("map LOS", btech_special_load_map_los);
  BTECH_LOAD("map child counts", btech_special_validate_map_child_counts);
  BTECH_LOAD("map objects", btech_special_load_map_objects);
  BTECH_LOAD("map bits", btech_special_load_map_bits);
  BTECH_LOAD("mech parents", btech_special_load_mech_parents);
  BTECH_LOAD("mech sections", btech_special_load_mech_sections);
  BTECH_LOAD("mech criticals", btech_special_load_mech_criticals);
  BTECH_LOAD("mech positions", btech_special_load_mech_positions);
  BTECH_LOAD("mech bays", btech_special_load_mech_bays);
  BTECH_LOAD("mech turrets", btech_special_load_mech_turrets);
  BTECH_LOAD("mech C3", btech_special_load_mech_c3);
  BTECH_LOAD("mech C3 nodes", btech_special_load_mech_c3_nodes);
  BTECH_LOAD("mech tics", btech_special_load_mech_tics);
  BTECH_LOAD("mech frequencies", btech_special_load_mech_frequencies);
  BTECH_LOAD("mech runtime", btech_special_load_mech_runtime);
  BTECH_LOAD("mech unit auxiliary", btech_special_load_mech_unit_aux);
  BTECH_LOAD("mech runtime auxiliary", btech_special_load_mech_runtime_unused);
  BTECH_LOAD("mech stagger damage", btech_special_load_mech_stagger_damage);
  BTECH_LOAD("mech repair consoles", btech_special_load_mechrep);
  BTECH_LOAD("turrets", btech_special_load_turrets);
  BTECH_LOAD("turret tics", btech_special_load_turret_tics);
  BTECH_LOAD("autopilots", btech_special_load_autopilots);
  BTECH_LOAD("autopilot commands", btech_special_load_autopilot_commands);
  BTECH_LOAD("autopilot paths", btech_special_load_autopilot_path);
  BTECH_LOAD("repair events", btech_special_load_repair_events);
#undef BTECH_LOAD
  return 0;
}

/* Open the completed core snapshot for the post-core BTech restoration step. */
int btech_persistence_load_special_state_path(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result = -1;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK) {
    log_error(LOG_ALWAYS, "BTP", "FAIL",
              "Cannot open SQLite BTech state from %s: %s", (char *)path,
              sqlite ? sqlite3_errmsg(sqlite) : strerror(errno));
  } else if (btech_special_load_all(sqlite) < 0) {
    log_error(LOG_ALWAYS, "BTP", "FAIL",
              "Invalid or incomplete SQLite BTech state in %s: %s",
              (char *)path, sqlite3_errmsg(sqlite));
  } else {
    result = 0;
  }
  if (sqlite)
    sqlite3_close(sqlite);
  return result;
}

#ifdef BT_ADVANCED_ECON

/* One in-memory price array and the first part ID it represents. */
typedef struct btech_economy_cost_set BTECH_ECONOMY_COST_SET;
struct btech_economy_cost_set {
  unsigned long long *costs;
  size_t count;
  int first_part;
};

extern unsigned long long int specialcost[SPECIALCOST_SIZE];
extern unsigned long long int ammocost[AMMOCOST_SIZE];
extern unsigned long long int weapcost[WEAPCOST_SIZE];
extern unsigned long long int cargocost[CARGOCOST_SIZE];
extern unsigned long long int bombcost[BOMBCOST_SIZE];
extern char *part_figure_out_name(int part);
extern int temp_brand_flag;

/* Each array corresponds to one contiguous range of canonical part IDs. */
static BTECH_ECONOMY_COST_SET economy_cost_sets[] = {
    {specialcost, SPECIALCOST_SIZE, SPECIAL_BASE_INDEX},
    {ammocost, AMMOCOST_SIZE, AMMO_BASE_INDEX},
    {weapcost, WEAPCOST_SIZE, WEAPON_BASE_INDEX},
    {cargocost, CARGOCOST_SIZE, CARGO_BASE_INDEX},
    {bombcost, BOMBCOST_SIZE, BOMB_BASE_INDEX},
};

/* Execute a statement that does not return rows. */
static int btech_sqlite_exec(sqlite3 *sqlite, const char *sql) {
  char *error;
  int rc;

  error = NULL;
  rc = sqlite3_exec(sqlite, sql, NULL, NULL, &error);
  if (error)
    sqlite3_free(error);
  return rc == SQLITE_OK ? 0 : -1;
}

/* Bind and execute one reusable insert, leaving it ready for the next row. */
static int btech_sqlite_step(sqlite3_stmt *statement) {
  if (btech_special_test_should_fail(sqlite3_sql(statement), "step") ||
      sqlite3_step(statement) != SQLITE_DONE ||
      sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

/* Return whether this snapshot contains the BTech economy extension table. */
static int btech_economy_table_exists(sqlite3 *sqlite, int *exists) {
  sqlite3_stmt *statement;
  int step;
  int result;

  statement = NULL;
  result = -1;
  if (sqlite3_prepare_v2(sqlite,
                         "SELECT 1 FROM sqlite_master WHERE type = 'table' "
                         "AND name = 'btech_economy_costs';",
                         -1, &statement, NULL) == SQLITE_OK) {
    step = sqlite3_step(statement);
    if (step == SQLITE_ROW || step == SQLITE_DONE) {
      *exists = step == SQLITE_ROW;
      result = 0;
    }
  }
  sqlite3_finalize(statement);
  return result;
}

/* Return whether an existing table uses the current name-keyed schema. */
static int btech_economy_table_has_item_name(sqlite3 *sqlite, int *has_name) {
  sqlite3_stmt *statement;
  const unsigned char *column;
  int result;
  int step;

  statement = NULL;
  *has_name = 0;
  result = -1;
  if (sqlite3_prepare_v2(sqlite, "PRAGMA table_info(btech_economy_costs);", -1,
                         &statement, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      column = sqlite3_column_text(statement, 1);
      if (column && !strcmp((const char *)column, "item_name"))
        *has_name = 1;
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Read a decimal SQLite value without narrowing the unsigned 64-bit price. */
static int btech_parse_cost(const unsigned char *text,
                            unsigned long long *cost) {
  char *end;
  unsigned long long value;

  if (!text || text[0] == '-')
    return -1;
  errno = 0;
  value = strtoull((const char *)text, &end, 10);
  if (errno == ERANGE || end == (char *)text || *end)
    return -1;
  *cost = value;
  return 0;
}

/* Return an unbranded canonical name without needing runtime name hashes. */
static const char *btech_part_name(int part) {
  const char *item_name;
  int saved_brand_flag;

  saved_brand_flag = temp_brand_flag;
  temp_brand_flag = 0;
  item_name = part_figure_out_name(part);
  temp_brand_flag = saved_brand_flag;
  return item_name;
}

/* Resolve an unbranded canonical name without needing runtime name hashes. */
static int btech_part_from_name(const char *item_name, int *part) {
  BTECH_ECONOMY_COST_SET *cost_set;
  const char *candidate;
  size_t index;
  size_t item_index;
  int candidate_part;

  for (index = 0;
       index < sizeof(economy_cost_sets) / sizeof(economy_cost_sets[0]);
       index++) {
    cost_set = &economy_cost_sets[index];
    for (item_index = 0; item_index < cost_set->count; item_index++) {
      candidate_part = cost_set->first_part + item_index;
      candidate = btech_part_name(candidate_part);
      if (candidate && !strcmp(item_name, candidate)) {
        *part = candidate_part;
        return 1;
      }
    }
  }
  return 0;
}

/* Restore sparse named prices, leaving omitted parts at the zero default. */
static int btech_load_costs(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  const unsigned char *part_name;
  unsigned long long cost;
  int part;
  int result;
  int skipped;
  int step;

  statement = NULL;
  result = -1;
  skipped = 0;
  if (sqlite3_prepare_v2(sqlite,
                         "SELECT item_name, cost FROM btech_economy_costs;", -1,
                         &statement, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      part_name = sqlite3_column_text(statement, 0);
      if (!part_name || !btech_part_from_name((const char *)part_name, &part)) {
        skipped++;
      } else if (btech_parse_cost(sqlite3_column_text(statement, 1), &cost) <
                 0) {
        result = -1;
      } else {
        SetPartCost(part, cost);
      }
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  }
  sqlite3_finalize(statement);
  if (skipped)
    log_error(LOG_ALWAYS, "ECO", "INFO",
              "Ignored %d SQLite economy rows for parts unavailable in this "
              "build.",
              skipped);
  return result;
}

/* Restore economy prices from the SQLite game database. */
static int btech_persistence_load_economy(sqlite3 *sqlite) {
  size_t index;
  int exists;
  int has_item_name;

  for (index = 0;
       index < sizeof(economy_cost_sets) / sizeof(economy_cost_sets[0]);
       index++)
    memset(economy_cost_sets[index].costs, 0,
           economy_cost_sets[index].count *
               sizeof(*economy_cost_sets[index].costs));

  exists = 0;
  if (btech_economy_table_exists(sqlite, &exists) < 0)
    return -1;
  if (!exists) {
    log_error(LOG_ALWAYS, "ECO", "FAIL",
              "SQLite game database lacks required btech_economy_costs data.");
    return -1;
  }

  has_item_name = 0;
  if (btech_economy_table_has_item_name(sqlite, &has_item_name) < 0)
    return -1;
  if (!has_item_name) {
    log_error(LOG_ALWAYS, "ECO", "FAIL",
              "SQLite economy data lacks required item_name schema.");
    return -1;
  }

  return btech_load_costs(sqlite);
}

/* Write non-default advanced-economy prices in the core snapshot transaction.
 */
static int btech_persistence_store_economy(sqlite3 *sqlite) {
  BTECH_ECONOMY_COST_SET *cost_set;
  sqlite3_stmt *statement;
  const char *part_name;
  int part;
  size_t index;
  size_t item_index;
  char cost[32];
  int length;
  int result;

  statement = NULL;
  if (btech_sqlite_exec(sqlite, "CREATE TABLE btech_economy_costs ("
                                " item_name TEXT PRIMARY KEY,"
                                " cost TEXT NOT NULL"
                                ") WITHOUT ROWID;") < 0 ||
      sqlite3_prepare_v2(sqlite,
                         "INSERT INTO btech_economy_costs (item_name, cost) "
                         "VALUES (?, ?);",
                         -1, &statement, NULL) != SQLITE_OK)
    return -1;

  result = 0;
  for (index = 0; result == 0 && index < sizeof(economy_cost_sets) /
                                             sizeof(economy_cost_sets[0]);
       index++) {
    cost_set = &economy_cost_sets[index];
    for (item_index = 0; item_index < cost_set->count; item_index++) {
      if (!cost_set->costs[item_index])
        continue;
      part = cost_set->first_part + item_index;
      part_name = btech_part_name(part);
      length =
          snprintf(cost, sizeof(cost), "%llu", cost_set->costs[item_index]);
      if (!part_name || length < 0 || (size_t)length >= sizeof(cost) ||
          sqlite3_bind_text(statement, 1, part_name, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          sqlite3_bind_text(statement, 2, cost, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          btech_sqlite_step(statement) < 0) {
        result = -1;
        break;
      }
    }
  }
  sqlite3_finalize(statement);
  return result;
}
#endif

/* Register BTech's data tables without making core MUX depend on BTech data. */
int btech_persistence_register(void) {
  if (persistence_register_sqlite_extension(
          "btech_special_state", btech_persistence_preload_special_state,
          btech_persistence_store_special_state) < 0)
    return -1;
#ifdef BT_ADVANCED_ECON
  return persistence_register_sqlite_extension("btech_economy",
                                               btech_persistence_load_economy,
                                               btech_persistence_store_economy);
#else
  return 0;
#endif
}
