/* btech_persistence_sqlite.c -- BTech state in the MUX SQLite game database */

#include <errno.h>
#include <float.h>
#include <limits.h>
#include <math.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "autopilot.h"
#include "btconfig.h"
#include "btech/persistence.h"
#include "btmux_build_config.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_parts.h"
#include "mech_persistence.h"
#include "mech_stagger.h"
#include "mech_tech.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/utf8.h"
#include "registry_api.h"
#include "schema.h"
#include "special_object.h"
#include "template_api.h"
#include "turret.h"

extern const AutopilotCommandDefinition acom[AUTO_NUM_COMMANDS + 1];
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

extern const char btech_special_schema_sql[];

void btech_special_test_reset_fault(void);
int btech_special_prepare_v2(sqlite3 *sqlite, const char *sql, int byte_count,
                             sqlite3_stmt **statement, const char **tail);
int btech_special_exec(sqlite3 *sqlite, const char *sql);
int btech_special_step(sqlite3_stmt *statement);
int btech_special_bind_int(sqlite3_stmt *statement, int index,
                           sqlite3_int64 value);
int btech_special_bind_real(sqlite3_stmt *statement, int index, double value);
int btech_special_store_metadata(sqlite3 *sqlite);
int btech_special_column_int(sqlite3_stmt *statement, int column, int *value);
int btech_special_column_long(sqlite3_stmt *statement, int column, long *value);
int btech_special_column_uint(sqlite3_stmt *statement, int column,
                              unsigned int *value);
int btech_special_column_ulong(sqlite3_stmt *statement, int column,
                               unsigned long *value);
int btech_special_column_char(sqlite3_stmt *statement, int column, char *value);
int btech_special_column_uchar(sqlite3_stmt *statement, int column,
                               unsigned char *value);
int btech_special_column_short(sqlite3_stmt *statement, int column,
                               short *value);
int btech_special_column_ushort(sqlite3_stmt *statement, int column,
                                unsigned short *value);
int btech_special_column_dbref(GameDatabase *database, sqlite3_stmt *statement,
                               int column, DbRef *value);
int btech_special_column_time(sqlite3_stmt *statement, int column,
                              time_t *value);
int btech_special_column_real(sqlite3_stmt *statement, int column,
                              float *value);
int btech_special_column_text(sqlite3_stmt *statement, int column,
                              char *destination, size_t destination_size);
int btech_special_validate_metadata(sqlite3 *sqlite);

int btech_special_load_map_parents(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_map_hexes(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_map_slots(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_map_los(sqlite3 *sqlite, BtechContext *context);
int btech_special_validate_map_child_counts(sqlite3 *sqlite);
int btech_special_load_map_objects(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_map_bits(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_repair_events(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_parents(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_sections(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_criticals(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_positions(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_bays(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_turrets(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_c3(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_c3_nodes(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_tics(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_frequencies(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_runtime(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_unit_aux(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_mech_runtime_unused(sqlite3 *sqlite,
                                           BtechContext *context);
int btech_special_load_mech_stagger_damage(sqlite3 *sqlite,
                                           BtechContext *context);
int btech_special_load_mechrep(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_turrets(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_turret_tics(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_autopilots(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_autopilot_commands(sqlite3 *sqlite,
                                          BtechContext *context);
int btech_special_load_autopilot_path(sqlite3 *sqlite, BtechContext *context);

int btech_store_simple_object(void *key, void *data, int depth, void *argument);
int btech_store_map(void *key, void *data, int depth, void *argument);
void btech_finalize_object_statements(BTECH_OBJECT_STORE_CONTEXT *context);
int btech_persistence_store_special_state(sqlite3 *sqlite,
                                          PersistenceContext *persistence,
                                          void *extension_context);
int btech_persistence_load_economy(sqlite3 *sqlite,
                                   PersistenceContext *persistence,
                                   void *extension_context);
int btech_persistence_store_economy(sqlite3 *sqlite,
                                    PersistenceContext *persistence,
                                    void *extension_context);

#ifndef BTECH_PERSISTENCE_PREPARE_IMPLEMENTATION
#define sqlite3_prepare_v2 btech_special_prepare_v2
#endif
