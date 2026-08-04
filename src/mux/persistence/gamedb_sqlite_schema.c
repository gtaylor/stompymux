/* gamedb_sqlite.c -- SQLite game-database persistence */

#include "mux/server/platform.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <sqlite3.h>
#include <sys/stat.h>
#include <unistd.h>

#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/powers.h"
#include "mux/persistence/gamedb.h"
#include "mux/persistence/gamedb_sqlite_internal.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/utf8.h"
#include "mux/support/validation.h"

// Increment whenever the schema written by this module changes.
const int GAMEDB_SCHEMA_VERSION = 25;

// Identifies SQLite as the storage implementation in snapshot metadata.
const int GAMEDB_SOURCE_FORMAT_SQLITE = 1;

/*
 * Each file holds one complete game snapshot. Typed Lua object state is
 * normalized so a future incremental store can use the same representation.
 */
const char schema_objects_sql[] =
    "CREATE TABLE snapshot ("
    " id INTEGER PRIMARY KEY CHECK (id = 1),"
    " schema_version INTEGER NOT NULL,"
    " storage_format INTEGER NOT NULL,"
    " storage_version INTEGER NOT NULL,"
    " dump_type INTEGER NOT NULL,"
    " dump_time INTEGER NOT NULL,"
    " db_top INTEGER NOT NULL,"
    " min_size INTEGER NOT NULL,"
    " record_players INTEGER NOT NULL"
    ");"
    "CREATE TABLE objects ("
    " dbref INTEGER PRIMARY KEY,"
    " name TEXT NOT NULL,"
    " location INTEGER NOT NULL,"
    " zone INTEGER NOT NULL,"
    " contents INTEGER NOT NULL,"
    " exits INTEGER NOT NULL,"
    " link INTEGER NOT NULL,"
    " next INTEGER NOT NULL,"
    " type INTEGER NOT NULL CHECK (type IN (0, 1, 2, 3, 5)),"
    " lua_parent TEXT NOT NULL DEFAULT '',"
    " description TEXT, inside_description TEXT,"
    " destroyer INTEGER,"
    " has_ansi_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_ansi_flag IN (0, 1)),"
    " has_audible_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_audible_flag IN "
    "(0, 1)),"
    " has_auditorium_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_auditorium_flag IN (0, 1)),"
    " has_blind_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_blind_flag IN (0, "
    "1)),"
    " has_connected_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_connected_flag "
    "IN (0, 1)),"
    " has_dark_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_dark_flag IN (0, 1)),"
    " has_floating_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_floating_flag IN "
    "(0, 1)),"
    " has_gagged_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_gagged_flag IN (0, "
    "1)),"
    " has_going_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_going_flag IN (0, "
    "1)),"
    " has_halted_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_halted_flag IN (0, "
    "1)),"
    " has_in_character_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_in_character_flag IN (0, 1)),"
    " has_light_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_light_flag IN (0, "
    "1)),"
    " has_monitor_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_monitor_flag IN "
    "(0, 1)),"
    " has_no_command_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_no_command_flag IN (0, 1)),"
    " has_quiet_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_quiet_flag IN (0, "
    "1)),"
    " has_safe_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_safe_flag IN (0, 1)),"
    " has_suspect_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_suspect_flag IN "
    "(0, 1)),"
    " has_transparent_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_transparent_flag IN (0, 1)),"
    " has_wizard_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_wizard_flag IN (0, "
    "1)),"
    " has_xcode_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_xcode_flag IN (0, "
    "1)),"
    " has_zombie_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_zombie_flag IN (0, "
    "1)),"
    " has_idle_power INTEGER NOT NULL DEFAULT 0 CHECK (has_idle_power IN (0, "
    "1))"
    ");";

const char schema_state_sql[] =
    "CREATE TABLE player_state ("
    " object_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref),"
    " password_hash TEXT, alias TEXT, last_login TEXT, last_name_change TEXT,"
    " login_data TEXT, last_site TEXT, last_page TEXT, timeout INTEGER,"
    " queue_limit INTEGER, privileges TEXT, timezone TEXT"
    ");"
    "CREATE TABLE btech_object_state ("
    " object_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref),"
    " mech_preferred_id TEXT, map_color TEXT, mech_skills TEXT,"
    " object_type TEXT, tactical_size TEXT, lrs_height TEXT,"
    " contact_options TEXT, mech_name TEXT, mech_type TEXT,"
    " mech_description TEXT, mw_template TEXT,"
    " faction TEXT, job TEXT, rank_number INTEGER, health TEXT,"
    " character_attributes TEXT, build_links TEXT, build_entrances TEXT,"
    " build_coordinates TEXT, advantages TEXT, pilot_dbref INTEGER,"
    " map_visibility TEXT, tech_complete_at INTEGER, economy_parts TEXT,"
    " skills TEXT, personal_combat_equipment TEXT"
    ");"
    "CREATE TABLE object_state ("
    " object_dbref INTEGER NOT NULL REFERENCES objects(dbref),"
    " namespace TEXT NOT NULL,"
    " key TEXT NOT NULL,"
    " value_type INTEGER NOT NULL CHECK (value_type BETWEEN 1 AND 4),"
    " value BLOB NOT NULL,"
    " PRIMARY KEY (object_dbref, namespace, key)"
    ") WITHOUT ROWID;";

const NativeColumn native_columns[] = {
    {A_DESC, "objects", "dbref", "description"},
    {A_IDESC, "objects", "dbref", "inside_description"},
    {A_DESTROYER, "objects", "dbref", "destroyer"},
    {A_PASS, "player_state", "object_dbref", "password_hash"},
    {A_ALIAS, "player_state", "object_dbref", "alias"},
    {A_LAST, "player_state", "object_dbref", "last_login"},
    {A_LASTNAME, "player_state", "object_dbref", "last_name_change"},
    {A_LOGINDATA, "player_state", "object_dbref", "login_data"},
    {A_LASTSITE, "player_state", "object_dbref", "last_site"},
    {A_LASTPAGE, "player_state", "object_dbref", "last_page"},
    {A_TIMEOUT, "player_state", "object_dbref", "timeout"},
    {A_QUEUEMAX, "player_state", "object_dbref", "queue_limit"},
    {A_PRIVS, "player_state", "object_dbref", "privileges"},
    {A_TZ, "player_state", "object_dbref", "timezone"},
    {A_MECHPREFID, "btech_object_state", "object_dbref", "mech_preferred_id"},
    {A_MAPCOLOR, "btech_object_state", "object_dbref", "map_color"},
    {A_MECHSKILLS, "btech_object_state", "object_dbref", "mech_skills"},
    {A_XTYPE, "btech_object_state", "object_dbref", "object_type"},
    {A_TACSIZE, "btech_object_state", "object_dbref", "tactical_size"},
    {A_LRSHEIGHT, "btech_object_state", "object_dbref", "lrs_height"},
    {A_CONTACTOPT, "btech_object_state", "object_dbref", "contact_options"},
    {A_MECHNAME, "btech_object_state", "object_dbref", "mech_name"},
    {A_MECHTYPE, "btech_object_state", "object_dbref", "mech_type"},
    {A_MECHDESC, "btech_object_state", "object_dbref", "mech_description"},
    {A_MWTEMPLATE, "btech_object_state", "object_dbref", "mw_template"},
    {A_FACTION, "btech_object_state", "object_dbref", "faction"},
    {A_JOB, "btech_object_state", "object_dbref", "job"},
    {A_RANKNUM, "btech_object_state", "object_dbref", "rank_number"},
    {A_HEALTH, "btech_object_state", "object_dbref", "health"},
    {A_ATTRS, "btech_object_state", "object_dbref", "character_attributes"},
    {A_BUILDLINKS, "btech_object_state", "object_dbref", "build_links"},
    {A_BUILDENTRANCE, "btech_object_state", "object_dbref", "build_entrances"},
    {A_BUILDCOORD, "btech_object_state", "object_dbref", "build_coordinates"},
    {A_ADVS, "btech_object_state", "object_dbref", "advantages"},
    {A_PILOTNUM, "btech_object_state", "object_dbref", "pilot_dbref"},
    {A_MAPVIS, "btech_object_state", "object_dbref", "map_visibility"},
    {A_TECHTIME, "btech_object_state", "object_dbref", "tech_complete_at"},
    {A_ECONPARTS, "btech_object_state", "object_dbref", "economy_parts"},
    {A_SKILLS, "btech_object_state", "object_dbref", "skills"},
    {A_PCEQUIP, "btech_object_state", "object_dbref",
     "personal_combat_equipment"},
};

const size_t native_column_count =
    sizeof(native_columns) / sizeof(*native_columns);

/* Log either the SQLite error or the current operating-system error. */
