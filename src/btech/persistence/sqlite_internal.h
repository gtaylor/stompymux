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
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_events.h"
#include "mech_parts.h"
#include "mech_persistence.h"
#include "mech_stagger.h"
#include "mech_tech_events_api.h"
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
#include "part_cost_api.h"
#include "registry_api.h"
#include "schema.h"
#include "special_object.h"
#include "template_api.h"
#include "turret.h"

/* Fault injection is owned by one in-progress snapshot write. */
typedef struct BtechSpecialWriteContext {
#ifdef BTECH_PERSISTENCE_TESTING
  const char *table;
  const char *phase;
  bool triggered;
#else
  bool disabled;
#endif
} BtechSpecialWriteContext;

typedef struct BtechMapStoreContext BtechMapStoreContext;
struct BtechMapStoreContext {
  BtechSpecialWriteContext *fault;
  sqlite3_stmt *map;
  sqlite3_stmt *hex;
  sqlite3_stmt *slot;
  sqlite3_stmt *los;
  sqlite3_stmt *object;
  sqlite3_stmt *bits;
  int result;
};

typedef struct BtechObjectStoreContext BtechObjectStoreContext;
struct BtechObjectStoreContext {
  BtechSpecialWriteContext *fault;
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
  sqlite3_stmt *unit_aux;
  sqlite3_stmt *stagger_damage;
  sqlite3_stmt *autopilot_command;
  sqlite3_stmt *autopilot_command_arg;
  sqlite3_stmt *autopilot_path;
  int result;
};

extern const char BTECH_SPECIAL_SCHEMA_SQL[];

void btech_special_write_context_init(BtechSpecialWriteContext *fault);
int btech_special_prepare_v2(sqlite3 *sqlite, const char *sql, int byte_count,
                             sqlite3_stmt **statement, const char **tail);
int btech_special_write_prepare(BtechSpecialWriteContext *fault,
                                sqlite3 *sqlite, const char *sql,
                                int byte_count, sqlite3_stmt **statement,
                                const char **tail);
int btech_special_exec(sqlite3 *sqlite, const char *sql);
int btech_special_step(sqlite3_stmt *statement);
int btech_special_write_step(BtechSpecialWriteContext *fault,
                             sqlite3_stmt *statement);
int btech_special_bind_int(sqlite3_stmt *statement, int index,
                           sqlite3_int64 value);
int btech_special_bind_real(sqlite3_stmt *statement, int index, double value);
int btech_special_store_metadata(BtechSpecialWriteContext *fault,
                                 sqlite3 *sqlite);
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
MuxEventCallback btech_special_repair_function_for_type(int type);
typedef enum BtechRepairEventClassification : int {
  BTECH_REPAIR_EVENT_INVALID,
  BTECH_REPAIR_EVENT_STALE,
  BTECH_REPAIR_EVENT_ACTIONABLE,
} BtechRepairEventClassification;
BtechRepairEventClassification
btech_special_repair_event_classify(Mech *mech, int event_type,
                                    intptr_t event_data, bool fake);
bool btech_special_repair_events_conflict(int first_type, intptr_t first_data,
                                          int second_type,
                                          intptr_t second_data);
int btech_special_store_repair_events(BtechSpecialWriteContext *fault,
                                      sqlite3_stmt *statement,
                                      BtechContext *context);
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
int btech_special_load_mech_stagger_damage(sqlite3 *sqlite,
                                           BtechContext *context);
int btech_special_load_mechrep(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_turrets(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_turret_tics(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_autopilots(sqlite3 *sqlite, BtechContext *context);
int btech_special_load_autopilot_commands(sqlite3 *sqlite,
                                          BtechContext *context);
int btech_special_load_autopilot_path(sqlite3 *sqlite, BtechContext *context);

bool btech_store_simple_object(const RedBlackTreeVisitCall *call);
bool btech_store_map(const RedBlackTreeVisitCall *call);
void btech_finalize_object_statements(BtechObjectStoreContext *context);
int btech_persistence_store_special_state(sqlite3 *sqlite,
                                          PersistenceContext *persistence,
                                          void *extension_context);
int btech_persistence_load_economy(sqlite3 *sqlite,
                                   PersistenceContext *persistence,
                                   void *extension_context);
int btech_persistence_store_economy(sqlite3 *sqlite,
                                    PersistenceContext *persistence,
                                    void *extension_context);
