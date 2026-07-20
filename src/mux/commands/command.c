/*
 * command.c - command parser and support routines
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "p.glue.h"

#include "mux/commands/command.h"
#include "mux/commands/functions.h"
#include "mux/commands/macro.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/database/vattr.h"
#include "mux/help/help_command.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"

#ifdef ARBITRARY_LOGFILES
#include "mux/server/log_cache.h"
#endif

constexpr char CACHING[] = "object";

/*
 * ---------------------------------------------------------------------------
 * * Switch tables for the various commands.
 */

// This sw may be spec'd w/others. Stored as int (not unsigned) so it ORs
// cleanly into NameTable.flag/CMDENT.extra without signedness conversions;
// C23 guarantees the top-bit pattern converts to INT_MIN deterministically.
constexpr int SW_MULTIPLE = (int)0x80000000U;
// Already have a unique option
constexpr int SW_GOT_UNIQUE = 0x40000000;
/*
 * (typically via a switch alias)
 */

NameTable attrib_sw[] = {{"access", 1, CA_GOD, ATTRIB_ACCESS},
                         {"delete", 1, CA_GOD, ATTRIB_DELETE},
                         {"rename", 1, CA_GOD, ATTRIB_RENAME},
                         {nullptr, 0, 0, 0}};

NameTable boot_sw[] = {{"port", 1, CA_WIZARD, BOOT_PORT | SW_MULTIPLE},
                       {"quiet", 1, CA_WIZARD, BOOT_QUIET | SW_MULTIPLE},
                       {nullptr, 0, 0, 0}};

NameTable chan_sw[] = {
    {"boot", 4, CA_PUBLIC, CHAN_BOOT | SW_MULTIPLE},
    {"create", 6, CA_PUBLIC, CHAN_CREATE | SW_MULTIPLE},
    {"destroy", 7, CA_PUBLIC, CHAN_DESTROY | SW_MULTIPLE},
    {"emit", 4, CA_PUBLIC, CHAN_EMIT | SW_MULTIPLE},
    {"list", 4, CA_PUBLIC, CHAN_LIST | SW_MULTIPLE},
    {"object", 6, CA_PUBLIC, CHAN_OBJECT | SW_MULTIPLE},
    {"oflags", 6, CA_PUBLIC, CHAN_OFLAGS | SW_MULTIPLE},
    {"pflags", 6, CA_PUBLIC, CHAN_PFLAGS | SW_MULTIPLE},
    {"flags", 5, CA_PUBLIC, CHAN_FLAGS | SW_MULTIPLE},
    {"status", 6, CA_PUBLIC, CHAN_STATUS | SW_MULTIPLE},
    {"who", 3, CA_PUBLIC, CHAN_WHO | SW_MULTIPLE},
    {"full", 4, CA_PUBLIC, CHAN_FULL | SW_MULTIPLE},
    {"noheader", 8, CA_PUBLIC, CHAN_NOHEADER | SW_MULTIPLE},
    {nullptr, 0, 0, 0},
};

NameTable lua_sw[] = {
    {"check", 5, CA_PUBLIC, LUA_COMMAND_CHECK},
    {"parent", 6, CA_PUBLIC, LUA_COMMAND_PARENT},
    {"reload", 6, CA_PUBLIC, LUA_COMMAND_RELOAD},
    {"schedule", 8, CA_PUBLIC, LUA_COMMAND_SCHEDULE},
    {nullptr, 0, 0, 0},
};

NameTable help_sw[] = {
    {"reload", 6, CA_PUBLIC, HELP_COMMAND_RELOAD},
    {nullptr, 0, 0, 0},
};

NameTable clone_sw[] = {
    {"inherit", 3, CA_PUBLIC, CLONE_INHERIT | SW_MULTIPLE},
    {"inventory", 3, CA_PUBLIC, CLONE_INVENTORY},
    {"location", 1, CA_PUBLIC, CLONE_LOCATION},
    {"parent", 2, CA_PUBLIC, CLONE_PARENT | SW_MULTIPLE},
    {"preserve", 2, CA_WIZARD, CLONE_PRESERVE | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

NameTable destroy_sw[] = {{"override", 8, CA_PUBLIC, DEST_OVERRIDE},
                          {"recursive", 9, CA_WIZARD, DEST_RECURSIVE},
                          {nullptr, 0, 0, 0}};

NameTable dig_sw[] = {{"teleport", 1, CA_PUBLIC, DIG_TELEPORT},
                      {nullptr, 0, 0, 0}};

NameTable dolist_sw[] = {{"delimit", 1, CA_PUBLIC, DOLIST_DELIMIT},
                         {"space", 1, CA_PUBLIC, DOLIST_SPACE},
                         {
                             nullptr,
                             0,
                             0,
                             0,
                         }};

NameTable drop_sw[] = {{"quiet", 1, CA_PUBLIC, DROP_QUIET}, {nullptr, 0, 0, 0}};

NameTable dump_sw[] = {{"structure", 1, CA_WIZARD, DUMP_STRUCT | SW_MULTIPLE},
                       {"text", 1, CA_WIZARD, DUMP_TEXT | SW_MULTIPLE},
                       {"optimize", 1, CA_WIZARD, DUMP_OPTIMIZE | SW_MULTIPLE},
                       {nullptr, 0, 0, 0}};

NameTable emit_sw[] = {{"here", 1, CA_PUBLIC, SAY_HERE | SW_MULTIPLE},
                       {"room", 1, CA_PUBLIC, SAY_ROOM | SW_MULTIPLE},
                       {nullptr, 0, 0, 0}};

NameTable enter_sw[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET},
                        {nullptr, 0, 0, 0}};

NameTable examine_sw[] = {{"brief", 1, CA_PUBLIC, EXAM_BRIEF},
                          {"debug", 1, CA_WIZARD, EXAM_DEBUG},
                          {"full", 1, CA_PUBLIC, EXAM_LONG},
                          {"parent", 1, CA_PUBLIC, EXAM_PARENT},
                          {nullptr, 0, 0, 0}};

NameTable femit_sw[] = {{"here", 1, CA_PUBLIC, PEMIT_HERE | SW_MULTIPLE},
                        {"room", 1, CA_PUBLIC, PEMIT_ROOM | SW_MULTIPLE},
                        {nullptr, 0, 0, 0}};

NameTable fixdb_sw[] = {
    /* {"add_pname",1,  CA_GOD,     FIXDB_ADD_PN}, */
    {"contents", 1, CA_GOD, FIXDB_CON},
    {"exits", 1, CA_GOD, FIXDB_EXITS},
    {"location", 1, CA_GOD, FIXDB_LOC},
    {"next", 1, CA_GOD, FIXDB_NEXT},
    {"owner", 1, CA_GOD, FIXDB_OWNER},
    {"rename", 1, CA_GOD, FIXDB_NAME},
    /* {"rm_pname", 1,  CA_GOD,     FIXDB_DEL_PN}, */
    {nullptr, 0, 0, 0}};

NameTable fpose_sw[] = {{"default", 1, CA_PUBLIC, 0},
                        {"nospace", 1, CA_PUBLIC, SAY_NOSPACE},
                        {nullptr, 0, 0, 0}};

NameTable function_sw[] = {{"privileged", 3, CA_WIZARD, FN_PRIV},
                           {"preserve", 3, CA_WIZARD, FN_PRES},
                           {nullptr, 0, 0, 0}};

NameTable get_sw[] = {{"quiet", 1, CA_PUBLIC, GET_QUIET}, {nullptr, 0, 0, 0}};

NameTable give_sw[] = {{"quiet", 1, CA_WIZARD, GIVE_QUIET}, {nullptr, 0, 0, 0}};

NameTable goto_sw[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET}, {nullptr, 0, 0, 0}};

NameTable halt_sw[] = {{"all", 1, CA_PUBLIC, HALT_ALL}, {nullptr, 0, 0, 0}};

NameTable leave_sw[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET},
                        {nullptr, 0, 0, 0}};

NameTable look_sw[] = {{"outside", 1, CA_PUBLIC, LOOK_OUTSIDE},
                       {nullptr, 0, 0, 0}};

NameTable notify_sw[] = {{"all", 1, CA_PUBLIC, NFY_NFYALL},
                         {"first", 1, CA_PUBLIC, NFY_NFY},
                         {nullptr, 0, 0, 0}};

NameTable open_sw[] = {{"inventory", 1, CA_PUBLIC, OPEN_INVENTORY},
                       {"location", 1, CA_PUBLIC, OPEN_LOCATION},
                       {nullptr, 0, 0, 0}};

NameTable pemit_sw[] = {
    {"contents", 1, CA_PUBLIC, PEMIT_CONTENTS | SW_MULTIPLE},
    {"object", 1, CA_PUBLIC, 0},
    {"silent", 1, CA_PUBLIC, 0},
    {"list", 1, CA_PUBLIC, PEMIT_LIST | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

NameTable pose_sw[] = {{"default", 1, CA_PUBLIC, 0},
                       {"nospace", 1, CA_PUBLIC, SAY_NOSPACE},
                       {nullptr, 0, 0, 0}};

NameTable ps_sw[] = {{"all", 1, CA_PUBLIC, PS_ALL | SW_MULTIPLE},
                     {"brief", 1, CA_PUBLIC, PS_BRIEF},
                     {"long", 1, CA_PUBLIC, PS_LONG},
                     {"summary", 1, CA_PUBLIC, PS_SUMM},
                     {nullptr, 0, 0, 0}};

NameTable set_sw[] = {{"quiet", 1, CA_PUBLIC, SET_QUIET}, {nullptr, 0, 0, 0}};

NameTable stats_sw[] = {{"all", 1, CA_PUBLIC, STAT_ALL},
                        {"me", 1, CA_PUBLIC, STAT_ME},
                        {"player", 1, CA_PUBLIC, STAT_PLAYER},
                        {nullptr, 0, 0, 0}};

NameTable sweep_sw[] = {
    {"commands", 3, CA_PUBLIC, SWEEP_COMMANDS | SW_MULTIPLE},
    {"connected", 3, CA_PUBLIC, SWEEP_CONNECT | SW_MULTIPLE},
    {"exits", 1, CA_PUBLIC, SWEEP_EXITS | SW_MULTIPLE},
    {"here", 1, CA_PUBLIC, SWEEP_HERE | SW_MULTIPLE},
    {"inventory", 1, CA_PUBLIC, SWEEP_ME | SW_MULTIPLE},
    {"listeners", 1, CA_PUBLIC, SWEEP_LISTEN | SW_MULTIPLE},
    {"players", 1, CA_PUBLIC, SWEEP_PLAYER | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

NameTable switch_sw[] = {{"all", 1, CA_PUBLIC, SWITCH_ANY},
                         {"default", 1, CA_PUBLIC, SWITCH_DEFAULT},
                         {"first", 1, CA_PUBLIC, SWITCH_ONE},
                         {nullptr, 0, 0, 0}};

NameTable teleport_sw[] = {{"loud", 1, CA_PUBLIC, TELEPORT_DEFAULT},
                           {"quiet", 1, CA_PUBLIC, TELEPORT_QUIET},
                           {nullptr, 0, 0, 0}};

NameTable trig_sw[] = {{"quiet", 1, CA_PUBLIC, TRIG_QUIET}, {nullptr, 0, 0, 0}};

NameTable wall_sw[] = {{"emit", 1, CA_WIZARD, SAY_WALLEMIT},
                       {"no_prefix", 1, CA_WIZARD, SAY_NOTAG | SW_MULTIPLE},
                       {"pose", 1, CA_WIZARD, SAY_WALLPOSE},
                       {"wizard", 1, CA_WIZARD, SAY_WIZSHOUT | SW_MULTIPLE},
                       {"admin", 1, CA_ADMIN, SAY_ADMINSHOUT},
                       {nullptr, 0, 0, 0}};

NameTable warp_sw[] = {{"check", 1, CA_WIZARD, TWARP_CLEAN | SW_MULTIPLE},
                       {"dump", 1, CA_WIZARD, TWARP_DUMP | SW_MULTIPLE},
                       {"idle", 1, CA_WIZARD, TWARP_IDLE | SW_MULTIPLE},
                       {"queue", 1, CA_WIZARD, TWARP_QUEUE | SW_MULTIPLE},
                       {nullptr, 0, 0, 0}};

/*
 * Implement the @@ (comment) command. Very cpu-intensive :-)
 */
void do_comment(DbRef player, DbRef cause, int key) {}

DEFINE_COMMAND_ADAPTER(do_comment)
#ifdef ARBITRARY_LOGFILES
#endif

/* ---------------------------------------------------------------------------
 * Command table: Definitions for builtin commands, used to build the command
 * hash table.
 *
 * Format:  Name		Switches	Permissions Needed
 *	Key (if any)	Calling Seq			Handler
 */

CMDENT command_table[] = {
    {"@@",
     nullptr,
     CA_WIZARD,
     0,
     CS_NO_ARGS,
     {.invoke = do_comment_command_adapter}},
    {"@addcommand",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG,
     {.invoke = do_addcommand}},
    {"@admin",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_admin}},
    {"@alias", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_alias}},
    {"@attribute",
     attrib_sw,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_attribute}},
    {"@boot",
     boot_sw,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_boot}},
    {"@chan",
     chan_sw,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_chan}},
    {"@chown",
     nullptr,
     CA_WIZARD,
     CHOWN_ONE,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_chown}},
    {"@chownall",
     nullptr,
     CA_WIZARD,
     CHOWN_ALL,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_chownall}},
    {"@chzone",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_chzone}},
    {"@clone",
     clone_sw,
     CA_WIZARD | CA_CONTENTS,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_clone}},
    {"@cpattr",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_ARGV,
     {.invoke = do_cpattr}},
    {"@create",
     nullptr,
     CA_WIZARD | CA_CONTENTS,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_create}},
    {"@cut",
     nullptr,
     CA_WIZARD | CA_LOCATION,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_cut}},
    {"@dbck", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_dbck}},
    {"@dbclean", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_dbclean}},
    {"@delcommand",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG,
     {.invoke = do_delcommand}},
    {"@destroy",
     destroy_sw,
     CA_WIZARD,
     DEST_ONE,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_destroy}},
    /*{"@destroyall", NULL, CA_WIZARD,
       DEST_ALL, CS_ONE_ARG, {.invoke = do_destroy}}, */
    {"@dig",
     dig_sw,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_ARGV | CS_INTERP,
     {.invoke = do_dig}},
    {"@disable",
     nullptr,
     CA_WIZARD,
     GLOB_DISABLE,
     CS_ONE_ARG,
     {.invoke = do_global}},
    {"@dolist",
     dolist_sw,
     CA_WIZARD | CA_GBL_INTERP,
     0,
     CS_TWO_ARG | CS_CMDARG | CS_NOINTERP | CS_STRIP_AROUND | CS_NO_MACRO,
     {.invoke = do_dolist}},
    {"@drain",
     nullptr,
     CA_WIZARD | CA_GBL_INTERP,
     NFY_DRAIN,
     CS_TWO_ARG,
     {.invoke = do_notify}},
    {"@dump", dump_sw, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_dump}},
    {"@edit",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_ARGV | CS_STRIP_AROUND,
     {.invoke = do_edit}},
    {"@emit",
     emit_sw,
     CA_WIZARD | CA_LOCATION,
     SAY_EMIT,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_say}},
    {"@enable",
     nullptr,
     CA_WIZARD,
     GLOB_ENABLE,
     CS_ONE_ARG,
     {.invoke = do_global}},
    {"@entrances",
     nullptr,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_entrances}},
    {"@femit",
     femit_sw,
     CA_WIZARD | CA_LOCATION,
     PEMIT_FEMIT,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_pemit}},
    {"@find",
     nullptr,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_find}},
    {"@fixdb",
     fixdb_sw,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_fixdb}},
    /*{"@fnd", NULL, 0, 0, CS_ONE_ARG|CS_UNPARSE, {.invoke =
       do_fnd_command_adapter}}, */
    {"@force",
     nullptr,
     CA_WIZARD | CA_GBL_INTERP,
     FRC_COMMAND,
     CS_TWO_ARG | CS_INTERP | CS_CMDARG | CS_NO_MACRO,
     {.invoke = do_force}},
    {"@fpose",
     fpose_sw,
     CA_WIZARD | CA_LOCATION,
     PEMIT_FPOSE,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_pemit}},
    {"@fsay",
     nullptr,
     CA_WIZARD | CA_LOCATION,
     PEMIT_FSAY,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_pemit}},
    {"@function",
     function_sw,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_function}},
    {"@halt",
     halt_sw,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_halt}},
    {"@help", help_sw, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_help_admin}},
    {"@kick",
     nullptr,
     CA_WIZARD,
     QUEUE_KICK,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_queue}},
    {"@last",
     nullptr,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_last}},
    {"@link",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_link}},
    {"@lua", lua_sw, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_lua}},
    {"@list",
     nullptr,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_list}},
    {"@listcommands",
     nullptr,
     CA_WIZARD,
     0,
     CS_ONE_ARG,
     {.invoke = do_listcommands}},
    {"@list_file",
     nullptr,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_list_file}},
#ifdef ARBITRARY_LOGFILES
    {"@log", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_log}},
#endif
    {"@mvattr",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_ARGV,
     {.invoke = do_mvattr}},
    {"@name",
     nullptr,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_name}},
    {"@newpassword",
     nullptr,
     CA_WIZARD,
     PASS_ANY,
     CS_TWO_ARG,
     {.invoke = do_newpassword}},
    {"@notify",
     notify_sw,
     CA_WIZARD | CA_GBL_INTERP,
     0,
     CS_TWO_ARG,
     {.invoke = do_notify}},
    {"@oemit",
     nullptr,
     CA_WIZARD | CA_LOCATION,
     PEMIT_OEMIT,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_pemit}},
    {"@open",
     open_sw,
     CA_WIZARD,
     0,
     CS_TWO_ARG | CS_ARGV | CS_INTERP,
     {.invoke = do_open}},
    {"@parent", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_parent}},
    {"@password",
     nullptr,
     CA_WIZARD,
     PASS_MINE,
     CS_TWO_ARG,
     {.invoke = do_password}},
    {"@pcreate",
     nullptr,
     CA_WIZARD,
     PCRE_PLAYER,
     CS_TWO_ARG,
     {.invoke = do_pcreate}},
    {"@pemit",
     pemit_sw,
     CA_WIZARD,
     PEMIT_PEMIT,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_pemit}},
    {"@npemit",
     pemit_sw,
     CA_WIZARD,
     PEMIT_PEMIT,
     CS_TWO_ARG | CS_UNPARSE,
     {.invoke = do_pemit}},
    {"@power", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_power}},
    {"@ps", ps_sw, CA_WIZARD, 0, CS_ONE_ARG | CS_INTERP, {.invoke = do_ps}},
    {"@readcache", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_readcache}},
    {"@robot",
     nullptr,
     CA_WIZARD | CA_PLAYER,
     PCRE_ROBOT,
     CS_TWO_ARG,
     {.invoke = do_pcreate}},
    {"@search",
     nullptr,
     CA_WIZARD,
     SRCH_SEARCH,
     CS_ONE_ARG | CS_NOINTERP,
     {.invoke = do_search}},
    {"@set", set_sw, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_set}},
    {"@shutdown", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_shutdown}},
    {"@stats",
     stats_sw,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_stats}},
    {"@sweep", sweep_sw, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_sweep}},
    {"@switch",
     switch_sw,
     CA_WIZARD | CA_GBL_INTERP,
     0,
     CS_TWO_ARG | CS_ARGV | CS_CMDARG | CS_NOINTERP | CS_STRIP_AROUND,
     {.invoke = do_switch}},
    {"@teleport",
     teleport_sw,
     CA_WIZARD,
     TELEPORT_DEFAULT,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_teleport}},
    {"@timewarp",
     warp_sw,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_timewarp}},
    {"@trigger",
     trig_sw,
     CA_WIZARD | CA_GBL_INTERP,
     0,
     CS_TWO_ARG | CS_ARGV,
     {.invoke = do_trigger}},
    {"@unlink",
     nullptr,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_unlink}},
    {"@wait",
     nullptr,
     CA_WIZARD | CA_GBL_INTERP,
     0,
     CS_TWO_ARG | CS_CMDARG | CS_NOINTERP | CS_STRIP_AROUND | CS_NO_MACRO,
     {.invoke = do_wait}},
    {"@wall",
     wall_sw,
     CA_WIZARD,
     SAY_SHOUT,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_say}},
    {"@wipe",
     wall_sw,
     CA_WIZARD,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_wipe}},
    {"@session", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_session}},
    {"@who", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_who}},
    {"addcom", nullptr, CA_NO_IC, 0, CS_TWO_ARG, {.invoke = do_addcom}},
    {"allcom", nullptr, CA_NO_IC, 0, CS_ONE_ARG, {.invoke = do_allcom}},
    {"comlist", nullptr, CA_NO_IC, 0, CS_NO_ARGS, {.invoke = do_comlist}},
    {"clearcom", nullptr, CA_NO_IC, 0, CS_NO_ARGS, {.invoke = do_clearcom}},
    {"delcom", nullptr, CA_NO_IC, 0, CS_ONE_ARG, {.invoke = do_delcom}},
    {"drop",
     drop_sw,
     CA_CONTENTS | CA_LOCATION,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_drop}},
    {"enter",
     enter_sw,
     CA_LOCATION,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_enter}},
    {"examine",
     examine_sw,
     0,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_examine}},
    {"get", get_sw, CA_LOCATION, 0, CS_ONE_ARG | CS_INTERP, {.invoke = do_get}},
    {"give",
     give_sw,
     CA_LOCATION,
     0,
     CS_TWO_ARG | CS_INTERP,
     {.invoke = do_give}},
    {"goto",
     goto_sw,
     CA_LOCATION,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_move}},
    {"help", nullptr, 0, 0, CS_ONE_ARG, {.invoke = do_help}},
    {"inventory",
     nullptr,
     0,
     LOOK_INVENTORY,
     CS_NO_ARGS,
     {.invoke = do_inventory}},
    {"leave",
     leave_sw,
     CA_LOCATION,
     0,
     CS_NO_ARGS | CS_INTERP,
     {.invoke = do_leave}},
    {"look",
     look_sw,
     CA_LOCATION,
     LOOK_LOOK,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_look}},
    {"page", nullptr, 0, 0, CS_TWO_ARG | CS_INTERP, {.invoke = do_page}},
    {"pose",
     pose_sw,
     CA_LOCATION,
     SAY_POSE,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_say}},
    {"quit", nullptr, 0, 0, CS_NO_ARGS, {.invoke = do_quit}},
    {"say",
     nullptr,
     CA_LOCATION,
     SAY_SAY,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_say}},
    {"think", nullptr, 0, 0, CS_ONE_ARG, {.invoke = do_think}},
    {"use",
     nullptr,
     CA_GBL_INTERP,
     0,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_use}},
    {"version", nullptr, 0, 0, CS_NO_ARGS, {.invoke = do_version}},
    {"+show", nullptr, CA_NO_IC, 0, CS_TWO_ARG, {.invoke = do_show}},
    {"+rolls", nullptr, 0, 0, CS_NO_ARGS, {.invoke = do_show_stat}},
    {"+charclear", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_charclear}},
    {"\\",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG | CS_INTERP,
     {.invoke = do_say}},
    {"#",
     nullptr,
     CA_GBL_INTERP | CF_DARK,
     0,
     CS_ONE_ARG | CS_INTERP | CS_CMDARG,
     {.invoke = do_force_prefixed}},
    {":",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG | CS_INTERP | CS_LEADIN,
     {.invoke = do_say}},
    {";",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG | CS_INTERP | CS_LEADIN,
     {.invoke = do_say}},
    {"\"",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG | CS_INTERP | CS_LEADIN,
     {.invoke = do_say}},
    {"&", nullptr, CF_DARK, 0, CS_TWO_ARG | CS_LEADIN, {.invoke = do_setvattr}},
    {(char *)nullptr, nullptr, 0, 0, 0, {nullptr}}};

void init_cmdtab(CommandRegistry *registry) {
  CMDENT *cp;
  Attribute *ap;
  char *p;
  const char *q;
  char *cbuff;

  hash_table_initialize(&registry->commands, 250 * HASH_FACTOR);

  /*
   * Load attribute-setting commands
   */

  cbuff = alloc_sbuf("init_cmdtab");
  for (ap = attr_table; ap->name; ap++) {
    if ((ap->flags & AF_NOCMD) == 0) {
      p = cbuff;
      *p++ = '@';
      for (q = ap->name; *q; p++, q++)
        *p = ToLower(*q);
      *p = '\0';
      cp = malloc(sizeof(CMDENT));
      cp->cmdname = (char *)strsave(cbuff);
      cp->perms = 0;
      cp->switches = nullptr;
      if (ap->flags & (AF_WIZARD | AF_MDARK)) {
        cp->perms |= CA_WIZARD;
      }
      cp->extra = ap->number;
      cp->callseq = CS_TWO_ARG;
      cp->handler.invoke = do_setattr;
      hash_table_add(cp->cmdname, (int *)cp, &registry->commands);
    }
  }
  free_sbuf(cbuff);

  /*
   * Load the builtin commands
   */

  for (cp = command_table; cp->cmdname; cp++)
    hash_table_add(cp->cmdname, (int *)cp, &registry->commands);

  set_prefix_cmds(registry);

  registry->goto_command = hash_table_find("goto", &registry->commands);
}

void set_prefix_cmds(CommandRegistry *registry) {
  int i;

  /*
   * Load the command prefix table.  Note - these commands can never *
   * * * * * * be typed in by a user because commands are lowercased *
   * before  * * * * the  * hash table is checked. The names are *
   * abbreviated to * * * minimise * * name checking time.
   */

  for (i = 0; i < A_USER_START; i++)
    registry->prefix_commands[i] = nullptr;
  registry->prefix_commands['"'] = hash_table_find("\"", &registry->commands);
  registry->prefix_commands[':'] = hash_table_find(":", &registry->commands);
  registry->prefix_commands[';'] = hash_table_find(";", &registry->commands);
  registry->prefix_commands['\\'] = hash_table_find("\\", &registry->commands);
  registry->prefix_commands['#'] = hash_table_find("#", &registry->commands);
  registry->prefix_commands['&'] = hash_table_find("&", &registry->commands);
  registry->prefix_commands['-'] = hash_table_find("-", &registry->commands);
  registry->prefix_commands['~'] = hash_table_find("~", &registry->commands);
}

/*
 * Returns 1 if player is in an IC location, 0 if not. Take into account
 * the ooc_comsys directive.
 */
int is_in_character_location(GameDatabase *database,
                             const ServerConfiguration *configuration,
                             DbRef player) {
  DbRef d = game_object_location(database, player);
  int z = 0;

  while (is_player(database, d)) {
    DbRef od = d;

    if ((d = game_object_location(database, d)) == od)
      break;
    if (z++ >= 100)
      break;
  }
  if (configuration->btech_ooc_comsys && !is_gagged(database, player))
    return 0;
  else if (is_in_character(database, d) || is_gagged(database, player))
    return 1;
  return 0;
} /* end In_IC_Loc() */

/*
 * ---------------------------------------------------------------------------
 * * check_access: Check if player has access to function.
 */

int check_access(GameDatabase *database,
                 const ServerConfiguration *configuration, DbRef player,
                 int mask) {
  int succ, fail;

  if (mask & CA_DISABLED)
    return 0;
  if (is_god(database, player) || configuration->is_initializing)
    return 1;

  succ = fail = 0;
  if (mask & CA_GOD)
    fail++;
  if (mask & CA_WIZARD) {
    if (is_wizard(database, player))
      succ++;
    else
      fail++;
  }
  if ((succ == 0) && (mask & CA_ADMIN)) {
    if (is_wizard(database, player))
      succ++;
    else
      fail++;
  }
  if ((succ == 0) && (mask & CA_ROBOT)) {
    if (is_robot_player(database, player))
      succ++;
    else
      fail++;
  }
  if (succ > 0)
    fail = 0;
  if (fail > 0)
    return 0;

  /*
   * Check for forbidden flags.
   */

  if (!is_wizard(database, player) &&
      (((mask & CA_NO_ROBOT) && is_robot_player(database, player)) ||
       ((mask & CA_NO_SUSPECT) && is_suspect(database, player)) ||
       (!configuration->btech_ooc_comsys && (mask & CA_NO_IC) &&
        is_in_character_location(database, configuration, player)) ||
       ((mask & CA_NO_IC) && is_gagged(database, player))))
    return 0;
  return 1;
}

static inline bool is_protected(CMDENT *cmdp, int x) { return cmdp->perms & x; }

static void command_invoke(CMDENT *command, CommandContext *context,
                           DbRef player, DbRef cause, int key, char *unparsed,
                           char *first, char *second, char **vector,
                           int vector_count, char **command_arguments,
                           int command_argument_count) {
  CommandInvocation invocation = {
      .context = context,
      .player = player,
      .cause = cause,
      .key = key,
      .unparsed = unparsed,
      .first = first,
      .second = second,
      .vector = vector,
      .vector_count = vector_count,
      .command_arguments = command_arguments,
      .command_argument_count = command_argument_count,
  };

  command->handler.invoke(&invocation);
}

/*
 * ---------------------------------------------------------------------------
 * * process_cmdent: Perform indicated command with passed args.
 */
static void process_cmdent(CommandContext *context, CMDENT *cmdp, char *switchp,
                           DbRef player, DbRef cause, int interactive,
                           char *arg, char *unp_command, char *cargs[],
                           int ncargs) {
  char *buf1 = nullptr, *buf2 = nullptr, tchar = '\x00', *bp = nullptr,
       *str = nullptr, *buff = nullptr, *s = nullptr, *j = nullptr,
       *new = nullptr;
  char *args[MAX_ARG];
  int nargs = 0, i = 0, fail = 0, interp = 0, key = 0, xkey = 0;
  long aflags = 0;
  size_t length = 0;
  DbRef aowner = 0;
  char *aargs[10];
  ADDENT *add = nullptr;

  memset(args, 0, sizeof(char *) * MAX_ARG);
  memset(aargs, 0, sizeof(char *) * 10);

  /*
   * Perform object type checks.
   */

  fail = 0;
  if (is_protected(cmdp, CA_LOCATION) &&
      !has_location(context->world->database, player))
    fail++;
  if (is_protected(cmdp, CA_CONTENTS) &&
      !has_contents(context->world->database, player))
    fail++;
  if (is_protected(cmdp, CA_PLAYER) &&
      (typeof_obj(context->world->database, player) != TYPE_PLAYER))
    fail++;
  if (fail > 0) {
    notify(&context->evaluation, player,
           "Command incompatible with invoker type.");
    return;
  }
  /*
   * Check global flags
   */

  if (is_protected(cmdp, CA_GBL_INTERP) &&
      !context->world->configuration->is_interpreter_enabled) {
    notify(&context->evaluation, player,
           "Sorry, queueing and triggering are not allowed now.");
    return;
  }
  key = cmdp->extra & ~SW_MULTIPLE;
  if (key & SW_GOT_UNIQUE) {
    i = 1;
    key = key & ~SW_GOT_UNIQUE;
  } else {
    i = 0;
  }

  /*
   * Check if we have permission to execute the command
   */

  /* Asumption: base command permission required for all sub-commands */
  if (!check_access(context->world->database, context->world->configuration,
                    player, cmdp->perms)) {
    notify(&context->evaluation, player, "Permission denied.");
    return;
  }

  /*
   * Check command switches.  Note that there may be more than one, * *
   *
   * *  * *  * *  * * and that we OR all of them together along with
   * the * extra * value * * * from * the command table to produce the
   * key * value in * the handler * *  * call.
   */

  if (switchp && cmdp->switches) {
    do {
      buf1 = (char *)index(switchp, '/');
      if (buf1)
        *buf1++ = '\0';
      xkey = name_table_search(context->world->database,
                               context->world->configuration, player,
                               cmdp->switches, switchp);
      if (xkey == -1) {
        notify_printf(&context->evaluation, player,
                      "Unrecognized switch '%s' for command '%s'.", switchp,
                      cmdp->cmdname);
        return;
      } else if (xkey == -2) {
        notify(&context->evaluation, player, "Permission denied.");
        return;
      } else if (!(xkey & SW_MULTIPLE)) {
        if (i == 1) {
          notify(&context->evaluation, player,
                 "Illegal combination of switches.");
          return;
        }
        i = 1;
      } else {
        xkey &= ~SW_MULTIPLE;
      }
      key |= xkey;
      switchp = buf1;
    } while (buf1);
  } else if (switchp && !(cmdp->callseq & CS_ADDED)) {
    notify_printf(&context->evaluation, player,
                  "Command %s does not take switches.", cmdp->cmdname);
    return;
  }
  /*
   * We are allowed to run the command.  Now, call the handler using
   * the appropriate calling sequence and arguments.
   */

  if ((cmdp->callseq & CS_INTERP) ||
      !(interactive || (cmdp->callseq & CS_NOINTERP)))
    interp = EV_EVAL | EV_STRIP;
  else if (cmdp->callseq & CS_STRIP)
    interp = EV_STRIP;
  else if (cmdp->callseq & CS_STRIP_AROUND)
    interp = EV_STRIP_AROUND;
  else
    interp = 0;

  switch (cmdp->callseq & CS_NARG_MASK) {
  case CS_NO_ARGS: /*
                    * <cmd>   (no args)
                    */
    command_invoke(cmdp, context, player, cause, key, unp_command, nullptr,
                   nullptr, nullptr, 0, nullptr, 0);
    break;
  case CS_ONE_ARG: /*
                    * <cmd> <arg>
                    */

    /*
     * If an unparsed command, just give it to the handler
     */

    if (cmdp->callseq & CS_UNPARSE) {
      command_invoke(cmdp, context, player, cause, key, unp_command, nullptr,
                     nullptr, nullptr, 0, nullptr, 0);
      break;
    }
    /* Interpret if necessary, but not twice for CS_ADDED */

    if ((interp & EV_EVAL) && !(cmdp->callseq & CS_ADDED)) {
      buf1 = bp = alloc_lbuf("process_cmdent");
      str = arg;
      exec(&context->evaluation, buf1, &bp, 0, player, cause,
           interp | EV_FCHECK | EV_TOP, &str, cargs, ncargs);
      length = strnlen(buf1, LBUF_SIZE - 1);
      buf1[length] = '\0';
    } else
      buf1 =
          parse_to(context->world->configuration, &arg, '\0', interp | EV_TOP);

    /*
     * Call the correct handler
     */

    if (cmdp->callseq & CS_CMDARG) {
      command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                     nullptr, cargs, ncargs, nullptr, 0);
    } else {
      if (cmdp->callseq & CS_ADDED) {
        for (add = cmdp->handler.added; add != nullptr; add = add->next) {
          buff = attribute_get(context->world->database, add->thing, add->atr,
                               &aowner, &aflags);
          /* Skip the '$' character, and the next */
          for (s = buff + 2; *s && (*s != ':'); s++)
            ;
          if (!*s)
            break;
          *s++ = '\0';
          if (!(cmdp->callseq & CS_LEADIN))
            for (j = unp_command; *j && (*j != ' '); j++)
              ;
          else
            for (j = unp_command; *j; j++)
              ;

          new = alloc_lbuf("process_cmdent.soft");
          bp = new;
          if (!*j) {
            /* No args */
            if (!(cmdp->callseq & CS_LEADIN)) {
              safe_str(cmdp->cmdname, new, &bp);
            } else {
              safe_str(unp_command, new, &bp);
            }
            if (switchp) {
              safe_chr('/', new, &bp);
              safe_str(switchp, new, &bp);
            }
            *bp = '\0';
          } else {
            j++;
            safe_str(cmdp->cmdname, new, &bp);
            if (switchp) {
              safe_chr('/', new, &bp);
              safe_str(switchp, new, &bp);
            }
            safe_chr(' ', new, &bp);
            safe_str(j, new, &bp);
            *bp = '\0';
          }

          if (wild(buff + 1, new, aargs, 10)) {
            wait_que(context->runtime->commands, add->thing, player, 0, NOTHING,
                     0, s, aargs, 10, context->evaluation.registers);
            for (i = 0; i < 10; i++) {
              if (aargs[i])
                free_lbuf(aargs[i]);
            }
          }
          free_lbuf(new);
          free_lbuf(buff);
        }
      } else
        command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                       nullptr, nullptr, 0, nullptr, 0);
    }

    /*
     * Free the buffer if one was allocated
     */

    if (interp & EV_EVAL)
      free_lbuf(buf1);

    break;
  case CS_TWO_ARG: /*
                    * <cmd> <arg1> = <arg2>
                    */

    /*
     * Interpret ARG1
     */

    buf2 = parse_to(context->world->configuration, &arg, '=', EV_STRIP_TS);

    /*
     * Handle when no '=' was specified
     */

    if (!arg || (arg && !*arg)) {
      arg = &tchar;
      *arg = '\0';
    }
    buf1 = bp = alloc_lbuf("process_cmdent.2");
    str = buf2;
    exec(&context->evaluation, buf1, &bp, 0, player, cause,
         EV_STRIP | EV_FCHECK | EV_EVAL | EV_TOP, &str, cargs, ncargs);
    length = strnlen(buf1, LBUF_SIZE - 1);
    buf1[length] = '\0';

    if (cmdp->callseq & CS_ARGV) {

      /*
       * Arg2 is ARGV style.  Go get the args
       */

      parse_arglist(&context->evaluation, player, cause, arg, '\0',
                    interp | EV_STRIP_LS | EV_STRIP_TS, args, MAX_ARG, cargs,
                    ncargs);
      for (nargs = 0; (nargs < MAX_ARG) && args[nargs]; nargs++)
        ;

      /*
       * Call the correct command handler
       */

      if (cmdp->callseq & CS_CMDARG) {
        command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                       nullptr, args, nargs, cargs, ncargs);
      } else {
        command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                       nullptr, args, nargs, nullptr, 0);
      }

      /*
       * Free the argument buffers
       */

      for (i = 0; i <= nargs; i++)
        if (args[i])
          free_lbuf(args[i]);

    } else {

      /*
       * Arg2 is normal style.  Interpret if needed
       */

      if (interp & EV_EVAL) {
        buf2 = bp = alloc_lbuf("process_cmdent.3");
        str = arg;
        exec(&context->evaluation, buf2, &bp, 0, player, cause,
             interp | EV_FCHECK | EV_TOP, &str, cargs, ncargs);
        length = strnlen(buf2, LBUF_SIZE - 1);
        buf2[length] = '\0';
      } else if (cmdp->callseq & CS_UNPARSE) {
        buf2 = parse_to(context->world->configuration, &arg, '\0',
                        interp | EV_TOP | EV_NO_COMPRESS);
      } else {
        buf2 = parse_to(context->world->configuration, &arg, '\0',
                        interp | EV_STRIP_LS | EV_STRIP_TS | EV_TOP);
      }

      /*
       * Call the correct command handler
       */

      if (cmdp->callseq & CS_CMDARG) {
        command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                       buf2, cargs, ncargs, nullptr, 0);
      } else {
        command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                       buf2, nullptr, 0, nullptr, 0);
      }

      /*
       * Free the buffer, if needed
       */

      if (interp & EV_EVAL)
        free_lbuf(buf2);
    }

    /*
     * Free the buffer obtained by evaluating Arg1
     */

    free_lbuf(buf1);
    break;
  default:
    break;
  }
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * process_command: Execute a command.
 */

void process_command(CommandContext *context, char *command, char *args[],
                     int nargs) {
  CommandRuntime *runtime = context->runtime;
  ServerConfiguration *configuration = runtime->world->configuration;
  CommandRegistry *registry = runtime->command_registry;
  const DbRef player = context->player;
  const DbRef cause = context->enactor;
  const bool interactive = context->interactive;
  char *p = nullptr, *q = nullptr, *arg = nullptr, *lcbuf = nullptr,
       *slashp = nullptr, *bp = nullptr, *str = nullptr;
  const char *cmdsave = nullptr;
  long aflags = 0;
  int succ = 0, lua_succ = 0, i = 0;
  DbRef exit = 0, aowner = 0;
  CMDENT *cmdp = nullptr;
  char *macroout = nullptr;
  int macerr = 0;
  size_t length = 0;

  /*
   * Robustify player
   */

  cmdsave = context->debug_command;
  context->debug_command = "< process_command >";

  if (!command) {
    abort();
  }

  if (!is_good_obj(context->world->database, player)) {
    log_error(context->log, LOG_BUGS, "CMD", "PLYR",
              "Bad player in process_command: %ld", player);
    context->debug_command = cmdsave;
    goto exit;
  }

  /*
   * Make sure player isn't going or halted
   */

  if (is_going(context->world->database, player) ||
      (is_halted(context->world->database, player) &&
       !((typeof_obj(context->world->database, player) == TYPE_PLAYER) &&
         interactive))) {
    notify_printf(&context->evaluation,
                  game_object_owner(context->world->database, player),
                  "Attempt to execute command by halted object #%ld", player);
    context->debug_command = cmdsave;
    goto exit;
  }

  if (is_suspect(context->world->database, player)) {
    STARTLOG(context->log, LOG_SUSPECTCMDS | LOG_ALLCOMMANDS, "CMD", "SUS") {
      log_name_and_loc(context->log, player);
      lcbuf = alloc_lbuf("process_command.LOG.allcmds");
      snprintf(lcbuf, LBUF_SIZE, " entered: '%s'", command);
      log_text(lcbuf);
      free_lbuf(lcbuf);
      ENDLOG(context->log);
    }
    send_channel(
        &context->evaluation, "SuspectsLog", "%s (#%ld) (in #%ld) entered: %s",
        game_object_name(context->world->database, player), player,
        game_object_location(context->world->database, player), command);
  } else {
    STARTLOG(context->log, LOG_ALLCOMMANDS, "CMD", "ALL") {
      log_name_and_loc(context->log, player);
      lcbuf = alloc_lbuf("process_command.LOG.allcmds");
      snprintf(lcbuf, LBUF_SIZE, " entered: '%s'", command);
      log_text(lcbuf);
      free_lbuf(lcbuf);
      ENDLOG(context->log);
    }
  }

  /*
   * Reset recursion limits
   */
  command_context_reset_limits(context);

  if (is_verbose(context->world->database, player))
    notify_printf(&context->evaluation,
                  game_object_owner(context->world->database, player), "%s] %s",
                  game_object_name(context->world->database, player), command);

  /*
   * Eat leading whitespace, and space-compress if configured
   */

  while (*command && isspace(*command))
    command++;
  context->debug_command = command;

  /*
   * Can we fix the @npemit thing?
   */
  if (configuration->space_compress && strncmp(command, "@npemit", 7)) {
    p = q = command;
    while (*p) {
      while (*p && !isspace(*p))
        *q++ = *p++;
      while (*p && isspace(*p))
        p++;
      if (*p)
        *q++ = ' ';
    }
    *q = '\0';
  }

  /*
   * Now comes the fun stuff.  First check for single-letter leadins.
   * We check these before checking HOME because
   * they are among the most frequently executed commands,
   * and they can never be the HOME command.
   */

  i = command[0] & 0xff;
  if ((registry->prefix_commands[i] != nullptr) && command[0]) {
    process_cmdent(context, registry->prefix_commands[i], nullptr, player,
                   cause, interactive, command, command, args, nargs);
    context->debug_command = cmdsave;
    goto exit;
  }
  if (configuration->have_macros && (command[0] == '.') && interactive) {
    macerr = do_macro(&context->match, context->runtime->command_registry,
                      context->runtime->macros, player, command, &macroout);
    if (!macerr)
      goto exit;
    if (macerr == 1) {
      StringCopy(command, macroout);
      free_lbuf(macroout);
    }
  } else
    macerr = 0;
  if (configuration->have_comsys)
    if (!do_comsystem(&context->evaluation, player, command))
      goto exit;

  /* Handle mecha stuff.. */
  if (configuration->have_specials)
    if (HandledCommand(context->btech, player,
                       game_object_location(context->world->database, player),
                       command))
      goto exit;
  /*
   * Check for the HOME command
   */

  if (string_compare(configuration, command, "home") == 0) {
    if (((is_fixed(context->world->database, player)) ||
         (is_fixed(context->world->database,
                   game_object_owner(context->world->database, player)))) &&
        !(is_wizard(context->world->database, player))) {
      notify(&context->evaluation, player, configuration->fixed_home_msg);
      goto exit;
    }
    /* do_move()'s parameter isn't const-correct; "home" is only read. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    move_command(&context->evaluation, player, cause, 0, (char *)"home");
#pragma clang diagnostic pop
    context->debug_command = cmdsave;
    goto exit;
  }

  /*
   * Only check for exits if we may use the goto command
   */
  if (check_access(context->world->database, configuration, player,
                   ((CMDENT *)registry->goto_command)->perms)) {
    /*
     * Check for an exit name
     */
    init_match_check_keys(&context->match, player, command, TYPE_EXIT);
    match_exit_with_parents(&context->match);
    exit = last_match_result(&context->match);
    if (exit != NOTHING) {
      move_exit(&context->evaluation, player, exit, 0, "You can't go that way.",
                0);
      context->debug_command = cmdsave;
      goto exit;
    }
    /*
     * Check for an exit in the master room
     */

    init_match_check_keys(&context->match, player, command, TYPE_EXIT);
    match_master_exit(&context->match);
    exit = last_match_result(&context->match);
    if (exit != NOTHING) {
      move_exit(&context->evaluation, player, exit, 1, nullptr, 0);
      context->debug_command = cmdsave;
      goto exit;
    }
  }
  /*
   * Set up a lowercase command and an arg pointer for the hashed
   * command check.  Since some types of argument
   * processing destroy the arguments, make a copy so that
   * we keep the original command line intact.  Store the
   * edible copy in lcbuf after the lowercased command.
   */
  /*
   * Removed copy of the rest of the command, since it's ok do allow
   * it to be trashed.  -dcm
   */

  lcbuf = alloc_lbuf("process_commands.LCbuf");
  for (p = command, q = lcbuf; *p && !isspace(*p); p++, q++)
    *q = ToLower(*p); /*
                       * Make lowercase command
                       */
  *q++ = '\0';        /*
                       * Terminate command
                       */
  while (*p && isspace(*p))
    p++;   /*
            * Skip spaces before arg
            */
  arg = p; /*
            * Remember where arg starts
            */

  /*
   * Strip off any command switches and save them
   */

  slashp = (char *)index(lcbuf, '/');
  if (slashp)
    *slashp++ = '\0';

  /*
   * Check for a builtin command (or an alias of a builtin command)
   */

  cmdp = (CMDENT *)hash_table_find(lcbuf, &registry->commands);
  if (cmdp != nullptr) {
    if ((cmdp->callseq & CS_NO_MACRO) && macerr == 1)
      notify(&context->evaluation, player,
             "This command is unavailable as macro. Please use an "
             "attribute instead.");
    else
      process_cmdent(context, cmdp, slashp, player, cause, interactive, arg,
                     command, args, nargs);
    free_lbuf(lcbuf);
    context->debug_command = cmdsave;
    goto exit;
  }
  /*
   * Check for enter and leave aliases, user-defined commands on the *
   * * * * * * player, other objects where the player is, on objects in
   * * the  * *  * *  * player's inventory, and on the room that holds *
   * the * player. * We * *  * evaluate the command line here to allow *
   * chains  * of * $-commands to * * * work.
   */

  bp = lcbuf;
  str = command;
  exec(&context->evaluation, lcbuf, &bp, 0, player, cause,
       EV_EVAL | EV_FCHECK | EV_STRIP | EV_TOP, &str, args, nargs);
  length = strnlen(lcbuf, LBUF_SIZE - 1);
  lcbuf[length] = '\0';
  succ = 0;

  /*
   * Idea for enter/leave aliases from R'nice@TinyTIM
   */

  if (has_location(context->world->database, player) &&
      is_good_obj(context->world->database,
                  game_object_location(context->world->database, player))) {

    /* Check for a leave alias */
    p = attribute_parent_get(
        context->world->database,
        game_object_location(context->world->database, player), A_LALIAS,
        &aowner, &aflags);
    if (p && *p) {
      if (matches_exit_from_list(lcbuf, p)) {
        free_lbuf(lcbuf);
        free_lbuf(p);
        CommandInvocation invocation = {
            .context = context, .player = player, .cause = player};
        do_leave(&invocation);
        goto exit;
      }
    }
    free_lbuf(p);

    /*
     * Check for enter aliases
     */

    DOLIST(context->world->database, exit,
           game_object_contents(
               context->world->database,
               game_object_location(context->world->database, player))) {
      p = attribute_parent_get(context->world->database, exit, A_EALIAS,
                               &aowner, &aflags);
      if (p && *p) {
        if (matches_exit_from_list(lcbuf, p)) {
          free_lbuf(lcbuf);
          free_lbuf(p);
          do_enter_internal(&context->evaluation, player, exit, 0);
          goto exit;
        }
      }
      free_lbuf(p);
    }
  }
  /*
   * Lua is the first programmable-command stage.  It observes the original
   * unmatched command; local and zone legacy matching remains available when
   * no Lua handler accepts it.
   */
  if (configuration->match_mine &&
      !is_no_command(context->world->database, player) &&
      ((typeof_obj(context->world->database, player) != TYPE_PLAYER) ||
       configuration->match_mine_pl))
    lua_succ +=
        lua_command_match(runtime->lua_owner->runtime, context->descriptor,
                          player, player, cause, command);
  if (has_location(context->world->database, player)) {
    lua_succ += lua_list_command_match(
        runtime->lua_owner->runtime, context->descriptor,
        game_object_contents(
            context->world->database,
            game_object_location(context->world->database, player)),
        player, cause, command);
    if (!is_no_command(context->world->database,
                       game_object_location(context->world->database, player)))
      lua_succ += lua_command_match(
          runtime->lua_owner->runtime, context->descriptor,
          game_object_location(context->world->database, player), player, cause,
          command);
  }
  if (has_contents(context->world->database, player))
    lua_succ += lua_list_command_match(
        runtime->lua_owner->runtime, context->descriptor,
        game_object_contents(context->world->database, player), player, cause,
        command);
  if (!lua_succ && configuration->have_zones &&
      (game_object_zone(context->world->database,
                        game_object_location(context->world->database,
                                             player)) != NOTHING)) {
    if (typeof_obj(context->world->database,
                   game_object_zone(context->world->database,
                                    game_object_location(
                                        context->world->database, player))) ==
        TYPE_ROOM) {
      if (game_object_location(context->world->database, player) !=
          game_object_zone(context->world->database, player))
        lua_succ += lua_list_command_match(
            runtime->lua_owner->runtime, context->descriptor,
            game_object_contents(
                context->world->database,
                game_object_zone(
                    context->world->database,
                    game_object_location(context->world->database, player))),
            player, cause, command);
    } else if (!is_no_command(
                   context->world->database,
                   game_object_zone(context->world->database,
                                    game_object_location(
                                        context->world->database, player)))) {
      lua_succ += lua_command_match(
          runtime->lua_owner->runtime, context->descriptor,
          game_object_zone(
              context->world->database,
              game_object_location(context->world->database, player)),
          player, cause, command);
    }
  }
  if (!lua_succ && configuration->have_zones &&
      (game_object_zone(context->world->database, player) != NOTHING) &&
      !is_no_command(context->world->database,
                     game_object_zone(context->world->database, player)) &&
      (game_object_zone(
           context->world->database,
           game_object_location(context->world->database, player)) !=
       game_object_zone(context->world->database, player)))
    lua_succ +=
        lua_command_match(runtime->lua_owner->runtime, context->descriptor,
                          game_object_zone(context->world->database, player),
                          player, cause, command);
  if (lua_succ)
    succ = lua_succ;
  /*
   * Check for $-command matches on me
   */

  if (!lua_succ && configuration->match_mine &&
      (!(is_no_command(context->world->database, player)))) {
    if (((typeof_obj(context->world->database, player) != TYPE_PLAYER) ||
         configuration->match_mine_pl) &&
        (attribute_match(&context->evaluation, player, player, AMATCH_CMD,
                         lcbuf, 1) > 0)) {
      succ++;
    }
  }
  /*
   * Check for $-command matches on nearby things and on my room
   */

  if (!lua_succ && has_location(context->world->database, player)) {
    succ +=
        list_check(&context->evaluation,
                   game_object_contents(
                       context->world->database,
                       game_object_location(context->world->database, player)),
                   player, AMATCH_CMD, lcbuf, 1);

    if (!(is_no_command(
            context->world->database,
            game_object_location(context->world->database, player))))
      if (attribute_match(
              &context->evaluation,
              game_object_location(context->world->database, player), player,
              AMATCH_CMD, lcbuf, 1) > 0) {
        succ++;
      }
  }

  /*
   * Check for $-command matches in my inventory
   */

  if (!lua_succ && has_contents(context->world->database, player))
    succ += list_check(&context->evaluation,
                       game_object_contents(context->world->database, player),
                       player, AMATCH_CMD, lcbuf, 1);

  /*
   * now do check on zones
   */

  if (!lua_succ && (!succ) && configuration->have_zones &&
      (game_object_zone(context->world->database,
                        game_object_location(context->world->database,
                                             player)) != NOTHING)) {
    if (typeof_obj(context->world->database,
                   game_object_zone(context->world->database,
                                    game_object_location(
                                        context->world->database, player))) ==
        TYPE_ROOM) {

      /*
       * zone of player's location is a parent room
       */
      if (game_object_location(context->world->database, player) !=
          game_object_zone(context->world->database, player)) {

        /*
         * check parent room exits
         */
        init_match_check_keys(&context->match, player, command, TYPE_EXIT);
        match_zone_exit(&context->match);
        exit = last_match_result(&context->match);
        if (exit != NOTHING) {
          move_exit(&context->evaluation, player, exit, 1, nullptr, 0);
          context->debug_command = cmdsave;
          goto exit;
        }
        succ += list_check(
            &context->evaluation,
            game_object_contents(
                context->world->database,
                game_object_zone(
                    context->world->database,
                    game_object_location(context->world->database, player))),
            player, AMATCH_CMD, lcbuf, 1);
      } /*
         * * end of parent room checks
         */
    } else
      /*
       * try matching commands on area zone object
       */

      if ((!succ) && configuration->have_zones &&
          (game_object_zone(context->world->database,
                            game_object_location(context->world->database,
                                                 player)) != NOTHING) &&
          (!(is_no_command(
              context->world->database,
              game_object_zone(
                  context->world->database,
                  game_object_location(context->world->database, player))))))
        succ += attribute_match(
            &context->evaluation,
            game_object_zone(
                context->world->database,
                game_object_location(context->world->database, player)),
            player, AMATCH_CMD, lcbuf, 1);
  }
  /*
   * * end of matching on zone of player's * *
   * * * * * location
   */
  /*
   * if nothing matched with parent room/zone object, try matching
   * zone commands on the player's personal zone
   */
  if (!lua_succ && (!succ) && configuration->have_zones &&
      (game_object_zone(context->world->database, player) != NOTHING) &&
      (!(is_no_command(context->world->database,
                       game_object_zone(context->world->database, player)))) &&
      (game_object_zone(
           context->world->database,
           game_object_location(context->world->database, player)) !=
       game_object_zone(context->world->database, player))) {
    succ += attribute_match(&context->evaluation,
                            game_object_zone(context->world->database, player),
                            player, AMATCH_CMD, lcbuf, 1);
  }
  /*
   * Global Lua commands replace the master-room programmable-command stage.
   * They run only after every local and zone Lua or softcode command declined
   * the command. Master-room exits remain part of normal exit matching.
   */
  if (!lua_succ && !succ)
    succ +=
        lua_global_command_match(runtime->lua_owner->runtime,
                                 context->descriptor, player, cause, command);
  free_lbuf(lcbuf);

  /*
   * If we still didn't find anything, tell how to get help.
   */

  if (!succ) {
    notify(&context->evaluation, player, "Huh?  (Type \"help\" for help.)");
    STARTLOG(context->log, LOG_BADCOMMANDS, "CMD", "BAD") {
      log_name_and_loc(context->log, player);
      lcbuf = alloc_lbuf("process_commands.LOG.badcmd");
      snprintf(lcbuf, LBUF_SIZE, " entered: '%s'", command);
      log_text(lcbuf);
      free_lbuf(lcbuf);
      ENDLOG(context->log);
    }
  }
  context->debug_command = cmdsave;

exit:
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * list_cmdtable: List internal commands.
 */

static void list_cmdtable(EvaluationContext *evaluation,
                          const ServerConfiguration *configuration,
                          DbRef player) {
  CMDENT *cmdp;
  char *buf, *bp;
  const char *cp;

  buf = alloc_lbuf("list_cmdtable");
  bp = buf;
  for (cp = "Commands:"; *cp; cp++)
    *bp++ = *cp;
  for (cmdp = command_table; cmdp->cmdname; cmdp++) {
    if (check_access(evaluation->world->database, configuration, player,
                     cmdp->perms)) {
      if (!(cmdp->perms & CF_DARK)) {
        *bp++ = ' ';
        for (cp = cmdp->cmdname; *cp; cp++)
          *bp++ = *cp;
      }
    }
  }
  *bp = '\0';

  notify(evaluation, player, buf);
  free_lbuf(buf);
}

/*
 * ---------------------------------------------------------------------------
 * * list_attrtable: List available attributes.
 */

static void list_attrtable(EvaluationContext *evaluation, DbRef player) {
  Attribute *ap;
  char *buf, *bp;
  const char *cp;

  buf = alloc_lbuf("list_attrtable");
  bp = buf;
  for (cp = "Attributes:"; *cp; cp++)
    *bp++ = *cp;
  for (ap = attr_table; ap->name; ap++) {
    if (see_attr(evaluation, player, player, ap, player, 0)) {
      *bp++ = ' ';
      for (cp = ap->name; *cp; cp++)
        *bp++ = *cp;
    }
  }
  *bp = '\0';
  raw_notify(evaluation, player, buf);
  free_lbuf(buf);
}

/*
 * ---------------------------------------------------------------------------
 * * list_cmdaccess: List access commands.
 */

NameTable access_nametab[] = {{"god", 2, CA_GOD, CA_GOD},
                              {"wizard", 3, CA_WIZARD, CA_WIZARD},
                              {"robot", 2, CA_WIZARD, CA_ROBOT},
                              {"no_robot", 4, CA_WIZARD, CA_NO_ROBOT},
                              {"no_suspect", 5, CA_WIZARD, CA_NO_SUSPECT},
                              {"global_interp", 8, CA_PUBLIC, CA_GBL_INTERP},
                              {"disabled", 4, CA_GOD, CA_DISABLED},
                              {"need_location", 6, CA_PUBLIC, CA_LOCATION},
                              {"need_contents", 6, CA_PUBLIC, CA_CONTENTS},
                              {"need_player", 6, CA_PUBLIC, CA_PLAYER},
                              {"dark", 4, CA_GOD, CF_DARK},
                              {nullptr, 0, 0, 0}};

static void list_cmdaccess(EvaluationContext *evaluation,
                           const ServerConfiguration *configuration,
                           CommandRegistry *registry, DbRef player) {
  char *buff, *p;
  const char *q;
  CMDENT *cmdp;
  Attribute *ap;

  buff = alloc_sbuf("list_cmdaccess");
  for (cmdp = command_table; cmdp->cmdname; cmdp++) {
    if (check_access(evaluation->world->database, configuration, player,
                     cmdp->perms)) {
      if (!(cmdp->perms & CF_DARK)) {
        snprintf(buff, SBUF_SIZE, "%s:", cmdp->cmdname);
        name_table_list_set(evaluation, configuration, player, access_nametab,
                            cmdp->perms, buff, 1);
      }
    }
  }
  for (ap = attr_table; ap->name; ap++) {
    p = buff;
    *p++ = '@';
    for (q = ap->name; *q; p++, q++)
      *p = ToLower(*q);
    if (ap->flags & AF_NOCMD)
      continue;
    *p = '\0';
    cmdp = (CMDENT *)hash_table_find(buff, &registry->commands);
    if (cmdp == nullptr)
      continue;
    if (!check_access(evaluation->world->database, configuration, player,
                      cmdp->perms))
      continue;
    if (!(cmdp->perms & CF_DARK)) {
      snprintf(buff, SBUF_SIZE, "%s:", cmdp->cmdname);
      name_table_list_set(evaluation, configuration, player, access_nametab,
                          cmdp->perms, buff, 1);
    }
  }
  free_sbuf(buff);
}

/*
 * ---------------------------------------------------------------------------
 * * list_cmdswitches: List switches for commands.
 */

static void list_cmdswitches(EvaluationContext *evaluation,
                             const ServerConfiguration *configuration,
                             DbRef player) {
  char *buff;
  CMDENT *cmdp;

  buff = alloc_sbuf("list_cmdswitches");
  for (cmdp = command_table; cmdp->cmdname; cmdp++) {
    if (cmdp->switches) {
      if (check_access(evaluation->world->database, configuration, player,
                       cmdp->perms)) {
        if (!(cmdp->perms & CF_DARK)) {
          snprintf(buff, SBUF_SIZE, "%s:", cmdp->cmdname);
          name_table_display(evaluation, configuration, player, cmdp->switches,
                             buff, 0);
        }
      }
    }
  }
  free_sbuf(buff);
}
/* *INDENT-OFF* */

/* ---------------------------------------------------------------------------
 * list_attraccess: List access to attributes.
 */

NameTable attraccess_nametab[] = {{"dark", 2, CA_WIZARD, AF_DARK},
                                  {"deleted", 2, CA_WIZARD, AF_DELETED},
                                  {"god", 1, CA_PUBLIC, AF_GOD},
                                  {"hidden", 1, CA_WIZARD, AF_MDARK},
                                  {"ignore", 2, CA_WIZARD, AF_NOCMD},
                                  {"internal", 2, CA_WIZARD, AF_INTERNAL},
                                  {"no_command", 4, CA_PUBLIC, AF_NOPROG},
                                  {"no_inherit", 4, CA_PUBLIC, AF_PRIVATE},
                                  {"private", 1, CA_PUBLIC, AF_ODARK},
                                  {"regexp", 1, CA_PUBLIC, AF_REGEXP},
                                  {"visual", 1, CA_PUBLIC, AF_VISUAL},
                                  {"wizard", 1, CA_PUBLIC, AF_WIZARD},
                                  {nullptr, 0, 0, 0}};

NameTable indiv_attraccess_nametab[] = {
    {"hidden", 1, CA_WIZARD, AF_MDARK},
    {"wizard", 1, CA_WIZARD, AF_WIZARD},
    {"no_command", 4, CA_PUBLIC, AF_NOPROG},
    {"no_inherit", 4, CA_PUBLIC, AF_PRIVATE},
    {"visual", 1, CA_PUBLIC, AF_VISUAL},
    {"regexp", 1, CA_PUBLIC, AF_REGEXP},
    {nullptr, 0, 0, 0}};

/* *INDENT-ON* */

static void list_attraccess(EvaluationContext *evaluation,
                            const ServerConfiguration *configuration,
                            DbRef player) {
  char *buff;
  Attribute *ap;

  buff = alloc_sbuf("list_attraccess");
  for (ap = attr_table; ap->name; ap++) {
    if (read_attr(evaluation, player, player, ap, player, 0)) {
      snprintf(buff, SBUF_SIZE, "%s:", ap->name);
      name_table_list_set(evaluation, configuration, player, attraccess_nametab,
                          ap->flags, buff, 1);
    }
  }
  free_sbuf(buff);
}

/*
 * ---------------------------------------------------------------------------
 * * cf_access: Change command or switch permissions.
 */

int cf_access(int *vp, char *str, long extra, DbRef player, char *cmd,
              ConfigurationContext *context) {
  CMDENT *cmdp;
  char *ap;
  int set_switch;

  for (ap = str; *ap && !isspace(*ap) && (*ap != '/'); ap++)
    ;
  if (*ap == '/') {
    set_switch = 1;
    *ap++ = '\0';
  } else {
    set_switch = 0;
    if (*ap)
      *ap++ = '\0';
    while (*ap && isspace(*ap))
      ap++;
  }

  cmdp = (CMDENT *)hash_table_find(str, &context->command_registry->commands);
  if (cmdp != nullptr) {
    if (set_switch)
      return cf_ntab_access((int *)cmdp->switches, ap, extra, player, cmd,
                            context);
    else
      return configuration_modify_bits(&(cmdp->perms), ap, extra, player, cmd,
                                       context);
  } else {
    configuration_log_not_found(context, player, cmd, "Command", str);
    return -1;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * cf_acmd_access: Chante command permissions for all attr-setting cmds.
 */

int cf_acmd_access(int *vp, char *str, long extra, DbRef player, char *cmd,
                   ConfigurationContext *context) {
  CMDENT *cmdp;
  Attribute *ap;
  char *buff, *p;
  const char *q;
  int failure, save;

  buff = alloc_sbuf("cf_acmd_access");
  for (ap = attr_table; ap->name; ap++) {
    p = buff;
    *p++ = '@';
    for (q = ap->name; *q; p++, q++)
      *p = ToLower(*q);
    *p = '\0';
    cmdp =
        (CMDENT *)hash_table_find(buff, &context->command_registry->commands);
    if (cmdp != nullptr) {
      save = cmdp->perms;
      failure = configuration_modify_bits(&(cmdp->perms), str, extra, player,
                                          cmd, context);
      if (failure != 0) {
        cmdp->perms = save;
        free_sbuf(buff);
        return -1;
      }
    }
  }
  free_sbuf(buff);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_attr_access: Change access on an attribute.
 */

int cf_attr_access(int *vp, char *str, long extra, DbRef player, char *cmd,
                   ConfigurationContext *context) {
  Attribute *ap;
  char *sp;

  for (sp = str; *sp && !isspace(*sp); sp++)
    ;
  if (*sp)
    *sp++ = '\0';
  while (*sp && isspace(*sp))
    sp++;

  ap = attribute_by_name(context->database, str);
  if (ap != nullptr)
    return configuration_modify_bits(&(ap->flags), sp, extra, player, cmd,
                                     context);
  else {
    configuration_log_not_found(context, player, cmd, "Attribute", str);
    return -1;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * cf_cmd_alias: Add a command alias.
 */

int cf_cmd_alias(void *vp, char *str, long extra, DbRef player, char *cmd,
                 ConfigurationContext *context) {
  char *alias, *orig, *ap;
  CMDENT *cmdp, *cmd2;
  NameTable *nt;
  int *hp;

  alias = strtok(str, " \t=,");
  orig = strtok(nullptr, " \t=,");

  if (!orig) /*
              * * we only got one argument to @alias.
              * Bad.
              */
    return -1;

  for (ap = orig; *ap && (*ap != '/'); ap++)
    ;
  if (*ap == '/') {

    /*
     * Switch form of command aliasing: create an alias for a  *
     * * * * * * command + a switch
     */

    *ap++ = '\0';

    /*
     * Look up the command
     */

    cmdp = (CMDENT *)hash_table_find(orig, (HashTable *)vp);
    if (cmdp == nullptr) {
      configuration_log_not_found(context, player, cmd, "Command", orig);
      return -1;
    }
    /*
     * Look up the switch
     */

    nt = name_table_find_entry(context->database, context->configuration,
                               player, (NameTable *)cmdp->switches, ap);
    if (!nt) {
      configuration_log_not_found(context, player, cmd, "Switch", ap);
      return -1;
    }
    /*
     * Got it, create the new command table entry
     */

    cmd2 = malloc(sizeof(CMDENT));
    cmd2->cmdname = strsave(alias);
    cmd2->switches = cmdp->switches;
    cmd2->perms = cmdp->perms | nt->perm;
    cmd2->extra = (cmdp->extra | nt->flag) & ~SW_MULTIPLE;
    if (!(nt->flag & SW_MULTIPLE))
      cmd2->extra |= SW_GOT_UNIQUE;
    cmd2->callseq = cmdp->callseq;
    cmd2->handler = cmdp->handler;
    if (hash_table_add(cmd2->cmdname, (int *)cmd2, (HashTable *)vp)) {
      /* cmd2->cmdname was allocated by strsave() above; freeing it needs
         to discard the const we otherwise want on CMDENT.cmdname. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      free((void *)cmd2->cmdname);
#pragma clang diagnostic pop
      free(cmd2);
    }
  } else {

    /*
     * A normal (non-switch) alias
     */

    hp = hash_table_find(orig, (HashTable *)vp);
    if (hp == nullptr) {
      configuration_log_not_found(context, player, cmd, "Entry", orig);
      return -1;
    }
    hash_table_add(alias, hp, (HashTable *)vp);
  }
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * list_df_flags: List default flags at create time.
 */

static void list_df_flags(EvaluationContext *evaluation,
                          const ServerConfiguration *configuration,
                          DbRef player) {
  char *playerb, *roomb, *thingb, *exitb, *robotb, *buff;

  playerb = decode_flags(evaluation->world->database, player,
                         (configuration->player_flags.word1 | TYPE_PLAYER),
                         configuration->player_flags.word2,
                         configuration->player_flags.word3);
  roomb = decode_flags(evaluation->world->database, player,
                       (configuration->room_flags.word1 | TYPE_ROOM),
                       configuration->room_flags.word2,
                       configuration->room_flags.word3);
  exitb = decode_flags(evaluation->world->database, player,
                       (configuration->exit_flags.word1 | TYPE_EXIT),
                       configuration->exit_flags.word2,
                       configuration->exit_flags.word3);
  thingb = decode_flags(evaluation->world->database, player,
                        (configuration->thing_flags.word1 | TYPE_THING),
                        configuration->thing_flags.word2,
                        configuration->thing_flags.word3);
  robotb = decode_flags(evaluation->world->database, player,
                        (configuration->robot_flags.word1 | TYPE_PLAYER),
                        configuration->robot_flags.word2,
                        configuration->robot_flags.word3);
  buff = alloc_lbuf("list_df_flags");
  snprintf(buff, LBUF_SIZE,
           "Default flags: Players...%s Rooms...%s Exits...%s Things...%s "
           "Robots...%s",
           playerb, roomb, exitb, thingb, robotb);
  raw_notify(evaluation, player, buff);
  free_lbuf(buff);
  free_sbuf(playerb);
  free_sbuf(roomb);
  free_sbuf(exitb);
  free_sbuf(thingb);
  free_sbuf(robotb);
}

/*
 * ---------------------------------------------------------------------------
 * * list_options: List more game options from the context configuration.
 */

static const char *switchd[] = {"/first", "/all"};
static const char *examd[] = {"/brief", "/full"};
static const char *ed[] = {"Disabled", "Enabled"};

static void list_options(EvaluationContext *evaluation, CommandRuntime *runtime,
                         DbRef player) {
  const ServerConfiguration *configuration = runtime->world->configuration;
  char *buff;
  time_t now;

  now = time(nullptr);
  if (configuration->name_spaces)
    raw_notify(evaluation, player, "Player names may contain spaces.");
  else
    raw_notify(evaluation, player, "Player names may not contain spaces.");
  if (!configuration->robot_speak)
    raw_notify(evaluation, player,
               "Robots are not allowed to speak in public areas.");
  if (configuration->player_listen)
    raw_notify(evaluation, player,
               "The @Listen/@Ahear attribute set works on player objects.");
  if (configuration->ex_flags)
    raw_notify(
        evaluation, player,
        "The 'examine' command lists the flag names for the object's flags.");
  if (!configuration->quiet_look)
    raw_notify(evaluation, player,
               "The 'look' command shows visible attributes in "
               "addition to the description.");
  if (configuration->see_own_dark)
    raw_notify(evaluation, player,
               "The 'look' command lists DARK objects owned by you.");
  if (!configuration->dark_sleepers)
    raw_notify(evaluation, player,
               "The 'look' command shows disconnected players.");
  if (configuration->trace_topdown) {
    raw_notify(evaluation, player,
               "Trace output is presented top-down (whole expression "
               "first, then sub-exprs).");
    raw_notify(evaluation, player,
               tprintf("Only %d lines of trace output are displayed.",
                       configuration->trace_limit));
  } else {
    raw_notify(evaluation, player,
               "Trace output is presented bottom-up (subexpressions first).");
  }
  if (configuration->pemit_players)
    raw_notify(evaluation, player,
               "The '@pemit' command may be used to emit to faraway players.");
  if (configuration->pub_flags)
    raw_notify(evaluation, player,
               "The 'flags()' function will return the flags of any object.");
  if (configuration->read_rem_desc)
    raw_notify(
        evaluation, player,
        "The 'get()' function will return the description of faraway objects,");
  if (configuration->read_rem_name)
    raw_notify(
        evaluation, player,
        "The 'name()' function will return the name of faraway objects.");
  raw_notify(evaluation, player,
             tprintf("The default switch for the '@switch' command is %s.",
                     switchd[configuration->switch_df_all]));
  raw_notify(evaluation, player,
             tprintf("The default switch for the 'examine' command is %s.",
                     examd[configuration->exam_public]));
  if (configuration->sweep_dark)
    raw_notify(evaluation, player, "Players may @sweep dark locations.");
  if (configuration->fascist_tport)
    raw_notify(evaluation, player,
               "You may only @teleport out of locations you control.");
  raw_notify(
      evaluation, player,
      tprintf("Players may have at most %d commands in the queue at one time.",
              configuration->queuemax));
  if (configuration->match_mine) {
    if (configuration->match_mine_pl)
      raw_notify(evaluation, player,
                 "All objects search themselves for $-commands.");
    else
      raw_notify(
          evaluation, player,
          "Objects other than players search themselves for $-commands.");
  }
  if (!is_wizard(evaluation->world->database, player))
    return;
  buff = alloc_mbuf("list_options");

  raw_notify(
      evaluation, player,
      tprintf(
          "%d commands are run from the queue when there is no net activity.",
          configuration->queue_chunk));
  raw_notify(
      evaluation, player,
      tprintf("%d commands are run from the queue when there is net activity.",
              configuration->active_q_chunk));
  if (configuration->idle_wiz_dark)
    raw_notify(evaluation, player,
               "Wizards idle for longer than the default timeout are "
               "automatically set DARK.");
  if (configuration->safe_unowned)
    raw_notify(evaluation, player,
               "Objects not owned by you are automatically considered SAFE.");
  if (configuration->paranoid_alloc)
    raw_notify(evaluation, player,
               "The buffer pools are checked for consistency on each "
               "allocate or free.");
  raw_notify(evaluation, player,
             tprintf("The %s cache is %d entries wide by %d entries deep.",
                     CACHING, configuration->cache_width,
                     configuration->cache_depth));
  if (configuration->cache_names)
    raw_notify(evaluation, player, "A seperate name cache is used.");
  if (configuration->cache_trim)
    raw_notify(
        evaluation, player,
        "The cache depth is periodically trimmed back to its initial value.");
  if (configuration->fork_dump) {
    raw_notify(evaluation, player,
               "Database dumps are performed by a fork()ed process.");
    if (configuration->fork_vfork)
      raw_notify(evaluation, player,
                 "The 'vfork()' call is used to perform the fork.");
  }
  if (configuration->max_players >= 0)
    raw_notify(evaluation, player,
               tprintf("There may be at most %d players logged in at once.",
                       configuration->max_players));
  raw_notify(evaluation, player,
             tprintf("The head of the object freelist is #%ld.",
                     runtime->world->database->freelist));

  snprintf(buff, MBUF_SIZE, "Intervals: Dump...%d  Clean...%d  Idlecheck...%d",
           configuration->database.dump_interval, configuration->check_interval,
           configuration->idle_interval);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE, "Timers: Dump...%ld  Clean...%ld  Idlecheck...%ld",
           (long)runtime->clock->dump_deadline - now,
           (long)runtime->clock->check_deadline - now,
           (long)runtime->clock->idle_deadline - now);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE, "Timeouts: Idle...%d  Connect...%d  Tries...%d",
           configuration->idle_timeout, configuration->conn_timeout,
           configuration->retry_limit);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE,
           "Scheduling: Timeslice...%d  Max_Quota...%d  Increment...%d",
           configuration->timeslice, configuration->cmd_quota_max,
           configuration->cmd_quota_incr);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE, "Spaces...%s", ed[configuration->space_compress]);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE,
           "New characters: Room...#%d  Home...#%d  DefaultHome...#%d",
           configuration->start_room, configuration->start_home,
           configuration->default_home);
  raw_notify(evaluation, player, buff);

  snprintf(
      buff, MBUF_SIZE,
      "Misc: IdleQueueChunk...%d  ActiveQueueChunk...%d  Master_room...#%d",
      configuration->queue_chunk, configuration->active_q_chunk,
      configuration->master_room);
  raw_notify(evaluation, player, buff);

  free_mbuf(buff);
}

/*
 * ---------------------------------------------------------------------------
 * * list_db_stats: Get useful info from the DB layer about hash stats, etc.
 */
static void list_db_stats(EvaluationContext *evaluation, DbRef player) {
  raw_notify(evaluation, player, "Database is memory based.");
}

/*
 * ---------------------------------------------------------------------------
 * * list_process: List local resource usage stats of the mush process.
 * * Adapted from code by Claudius,
 * *     posted to the net by Howard/Dark_Lord.
 */

static void list_process(EvaluationContext *evaluation,
                         const RuntimeClock *clock, DbRef player) {
  int pid, psize, maxfds;

  struct rusage usage;
  int curr, last, dur;

  getrusage(RUSAGE_SELF, &usage);
  /*
   * Calculate memory use from the aggregate totals
   */

  curr = clock->current_sample;
  last = 1 - curr;
  dur = clock->sample_time[curr] - clock->sample_time[last];
  if (dur > 0) {
  }
  maxfds = getdtablesize();

  pid = getpid();
  psize = getpagesize();

  /*
   * Go display everything
   */

  raw_notify(
      evaluation, player,
      tprintf("Process ID:  %10d        %10d bytes per page", pid, psize));
  raw_notify(evaluation, player,
             tprintf("Time used:   %10ld user   %10ld sys",
                     usage.ru_utime.tv_sec, usage.ru_stime.tv_sec));

  /*
   * raw_notify(evaluation, player,
   * * tprintf("Resident mem:%10d shared %10d private%10d stack",
   * * ixrss, idrss, isrss));
   */
  raw_notify(evaluation, player,
             tprintf("Integral mem:%10ld shared %10ld private%10ld stack",
                     usage.ru_ixrss, usage.ru_idrss, usage.ru_isrss));
  raw_notify(evaluation, player,
             tprintf("Max res mem: %10ld pages  %10ld bytes", usage.ru_maxrss,
                     (usage.ru_maxrss * psize)));
  raw_notify(evaluation, player,
             tprintf("Page faults: %10ld hard   %10ld soft   %10ld swapouts",
                     usage.ru_majflt, usage.ru_minflt, usage.ru_nswap));
  raw_notify(evaluation, player,
             tprintf("Disk I/O:    %10ld reads  %10ld writes", usage.ru_inblock,
                     usage.ru_oublock));
  raw_notify(evaluation, player,
             tprintf("Network I/O: %10ld in     %10ld out", usage.ru_msgrcv,
                     usage.ru_msgsnd));
  raw_notify(evaluation, player,
             tprintf("Context swi: %10ld vol    %10ld forced %10ld sigs",
                     usage.ru_nvcsw, usage.ru_nivcsw, usage.ru_nsignals));
  raw_notify(evaluation, player, tprintf("Descs avail: %10d", maxfds));
}

/*
 * ---------------------------------------------------------------------------
 * * do_list: List information stored in internal structures.
 */

constexpr int LIST_ATTRIBUTES = 1;
constexpr int LIST_COMMANDS = 2;
constexpr int LIST_FLAGS = 4;
constexpr int LIST_FUNCTIONS = 5;
constexpr int LIST_GLOBALS = 6;
constexpr int LIST_LOGGING = 8;
constexpr int LIST_DF_FLAGS = 9;
constexpr int LIST_PERMS = 10;
constexpr int LIST_ATTRPERMS = 11;
constexpr int LIST_OPTIONS = 12;
constexpr int LIST_CONF_PERMS = 15;
constexpr int LIST_SITEINFO = 16;
constexpr int LIST_POWERS = 17;
constexpr int LIST_SWITCHES = 18;
constexpr int LIST_DB_STATS = 20;
constexpr int LIST_PROCESS = 21;
constexpr int LIST_BADNAMES = 22;
constexpr int LIST_LOGFILES = 23;

NameTable list_names[] = {{"attr_permissions", 5, CA_WIZARD, LIST_ATTRPERMS},
                          {"attributes", 2, CA_PUBLIC, LIST_ATTRIBUTES},
                          {"bad_names", 2, CA_WIZARD, LIST_BADNAMES},
                          {"commands", 3, CA_PUBLIC, LIST_COMMANDS},
                          {"config_permissions", 3, CA_GOD, LIST_CONF_PERMS},
                          {"db_stats", 2, CA_WIZARD, LIST_DB_STATS},
                          {"default_flags", 1, CA_PUBLIC, LIST_DF_FLAGS},
                          {"flags", 2, CA_PUBLIC, LIST_FLAGS},
                          {"functions", 2, CA_PUBLIC, LIST_FUNCTIONS},
                          {"globals", 1, CA_WIZARD, LIST_GLOBALS},
                          {"logging", 4, CA_GOD, LIST_LOGGING},
                          {"options", 1, CA_PUBLIC, LIST_OPTIONS},
                          {"permissions", 2, CA_WIZARD, LIST_PERMS},
                          {"powers", 2, CA_WIZARD, LIST_POWERS},
                          {"process", 2, CA_WIZARD, LIST_PROCESS},
                          {"site_information", 2, CA_WIZARD, LIST_SITEINFO},
                          {"switches", 2, CA_PUBLIC, LIST_SWITCHES},
#ifdef ARBITRARY_LOGFILES
                          {"logfiles", 4, CA_WIZARD, LIST_LOGFILES},
#endif
                          {nullptr, 0, 0, 0}};

extern NameTable logoptions_nametab[];
extern NameTable logdata_nametab[];

void do_list(CommandInvocation *invocation) {
  CommandRuntime *runtime = invocation->context->runtime;
  ServerConfiguration *configuration = runtime->world->configuration;
  const DbRef player = invocation->player;
  char *arg = invocation->first;
  int flagvalue;

  flagvalue = name_table_search(runtime->world->database, configuration, player,
                                list_names, arg);
  switch (flagvalue) {
  case LIST_ATTRIBUTES:
    list_attrtable(&invocation->context->evaluation, player);
    break;
  case LIST_COMMANDS:
    list_cmdtable(&invocation->context->evaluation, configuration, player);
    break;
  case LIST_SWITCHES:
    list_cmdswitches(&invocation->context->evaluation, configuration, player);
    break;
  case LIST_OPTIONS:
    list_options(&invocation->context->evaluation, runtime, player);
    break;
  case LIST_SITEINFO:
    list_siteinfo(&invocation->context->evaluation,
                  invocation->context->world->access_control, player);
    break;
  case LIST_FLAGS:
    display_flagtab(&invocation->context->evaluation, player);
    break;
  case LIST_FUNCTIONS:
    list_functable(&invocation->context->evaluation, configuration,
                   runtime->command_registry, player);
    break;
  case LIST_GLOBALS:
    list_global_controls(&invocation->context->evaluation, configuration,
                         player);
    break;
  case LIST_DF_FLAGS:
    list_df_flags(&invocation->context->evaluation, configuration, player);
    break;
  case LIST_PERMS:
    list_cmdaccess(&invocation->context->evaluation, configuration,
                   runtime->command_registry, player);
    break;
  case LIST_CONF_PERMS:
    configuration_list_access(&invocation->context->evaluation, player);
    break;
  case LIST_POWERS:
    display_powertab(&invocation->context->evaluation, player);
    break;
  case LIST_ATTRPERMS:
    list_attraccess(&invocation->context->evaluation, configuration, player);
    break;
  case LIST_LOGGING:
    name_table_interpret(&invocation->context->evaluation, configuration,
                         player, logoptions_nametab, configuration->log_options,
                         "Events Logged:", "enabled", "disabled");
    name_table_interpret(&invocation->context->evaluation, configuration,
                         player, logdata_nametab, configuration->log_info,
                         "Information Logged:", "yes", "no");
    break;
  case LIST_DB_STATS:
    list_db_stats(&invocation->context->evaluation, player);
    break;
  case LIST_PROCESS:
    list_process(&invocation->context->evaluation, runtime->clock, player);
    break;
  case LIST_BADNAMES:
    badname_list(&invocation->context->evaluation, invocation->context->world,
                 player, "Disallowed names:");
    break;
#ifdef ARBITRARY_LOGFILES
  case LIST_LOGFILES:
    log_cache_list(&invocation->context->evaluation,
                   invocation->context->log->cache, player);
    break;
#endif
  default:
    name_table_display(&invocation->context->evaluation, configuration, player,
                       list_names, "Unknown option.  Use one of:", 1);
  }
}
