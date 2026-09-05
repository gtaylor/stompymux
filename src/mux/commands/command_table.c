/*
 * command.c - command parser and support routines
 */

#include <stdlib.h>

#include "btech/character/btechstats_api.h"
#include "btech/context.h" // IWYU pragma: keep
#include "btech/ui/mech_stat_api.h"
#include "mux/commands/command.h"
#include "mux/commands/command_catalog.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_internal.h"
#include "mux/commands/command_keys.h"
#include "mux/commands/examine_commands.h"
#include "mux/commands/look.h"
#include "mux/commands/macro.h" // IWYU pragma: keep
#include "mux/commands/state_commands.h"
#include "mux/communication/comsys.h"
#include "mux/communication/speech.h"
#include "mux/help/help_command.h"
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/support/checked_storage.h"
#include "mux/support/name_table.h"
#include "mux/world/database_check.h"
#include "mux/world/inventory_commands.h"
#include "mux/world/movement_commands.h"
#include "mux/world/player.h"

/*
 * ---------------------------------------------------------------------------
 * * Switch tables for the various commands.
 */

/*
 * (typically via a switch alias)
 */

static const NameTable BOOT_SW[] = {
    {"port", 1, CA_WIZARD, BOOT_PORT | SW_MULTIPLE},
    {"quiet", 1, CA_WIZARD, BOOT_QUIET | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

static const NameTable BTECH_SW[] = {
    {"info", 1, CA_WIZARD, BTECH_INFO},
    {"register", 1, CA_WIZARD, BTECH_REGISTER},
    {"unregister", 1, CA_WIZARD, BTECH_UNREGISTER},
    {nullptr, 0, 0, 0}};

static const NameTable MECH_ADMIN_SW[] = {
    /* Full switch names avoid ambiguous repair, reload, restore, and set forms.
     */
    {"loadnew", 7, CA_WIZARD, MECH_ADMIN_LOADNEW},
    {"restore", 7, CA_WIZARD, MECH_ADMIN_RESTORE},
    {"savenew", 7, CA_WIZARD, MECH_ADMIN_SAVENEW},
    {"setarmor", 8, CA_WIZARD, MECH_ADMIN_SETARMOR},
    {"addweap", 7, CA_WIZARD, MECH_ADMIN_ADDWEAP},
    {"resetcrits", 10, CA_WIZARD, MECH_ADMIN_RESETCRITS},
    {"repair", 6, CA_WIZARD, MECH_ADMIN_REPAIR},
    {"reload", 6, CA_WIZARD, MECH_ADMIN_RELOAD},
    {"restock", 7, CA_WIZARD, MECH_ADMIN_RESTOCK},
    {"firemode", 8, CA_WIZARD, MECH_ADMIN_FIREMODE},
    {"addsp", 5, CA_WIZARD, MECH_ADMIN_ADDSP},
    {"display", 7, CA_WIZARD, MECH_ADMIN_DISPLAY},
    {"showtech", 8, CA_WIZARD, MECH_ADMIN_SHOWTECH},
    {"addtech", 7, CA_WIZARD, MECH_ADMIN_ADDTECH},
    {"deltech", 7, CA_WIZARD, MECH_ADMIN_DELTECH},
    {"addinftech", 10, CA_WIZARD, MECH_ADMIN_ADDINFTECH},
    {"delinftech", 10, CA_WIZARD, MECH_ADMIN_DELINFTECH},
    {"settons", 7, CA_WIZARD, MECH_ADMIN_SETTONS},
    {"settype", 7, CA_WIZARD, MECH_ADMIN_SETTYPE},
    {"setmove", 7, CA_WIZARD, MECH_ADMIN_SETMOVE},
    {"setmaxspeed", 11, CA_WIZARD, MECH_ADMIN_SETMAXSPEED},
    {"setheatsinks", 12, CA_WIZARD, MECH_ADMIN_SETHEATSINKS},
    {"setjumpspeed", 12, CA_WIZARD, MECH_ADMIN_SETJUMPSPEED},
    {"setlrsrange", 11, CA_WIZARD, MECH_ADMIN_SETLRSRANGE},
    {"settacrange", 11, CA_WIZARD, MECH_ADMIN_SETTACRANGE},
    {"setscanrange", 12, CA_WIZARD, MECH_ADMIN_SETSCANRANGE},
    {"setradio", 8, CA_WIZARD, MECH_ADMIN_SETRADIO},
    {"setradiorange", 13, CA_WIZARD, MECH_ADMIN_SETRADIORANGE},
    {"setcargospace", 13, CA_WIZARD, MECH_ADMIN_SETCARGOSPACE},
    {nullptr, 0, 0, 0}};

static const NameTable CHAN_SW[] = {
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

static const NameTable LUA_SW[] = {
    {"check", 5, CA_PUBLIC, LUA_COMMAND_CHECK},
    {"parent", 6, CA_PUBLIC, LUA_COMMAND_PARENT},
    {"reload", 6, CA_PUBLIC, LUA_COMMAND_RELOAD},
    {"schedule", 8, CA_PUBLIC, LUA_COMMAND_SCHEDULE},
    {"test", 4, CA_PUBLIC, LUA_COMMAND_TEST | SW_MULTIPLE},
    {"unit", 4, CA_PUBLIC, LUA_COMMAND_TEST_UNIT | SW_MULTIPLE},
    {"integration", 11, CA_PUBLIC, LUA_COMMAND_TEST_INTEGRATION | SW_MULTIPLE},
    {"verbose", 7, CA_PUBLIC, LUA_COMMAND_TEST_VERBOSE | SW_MULTIPLE},
    {"viewparent", 10, CA_PUBLIC, LUA_COMMAND_VIEWPARENT},
    {nullptr, 0, 0, 0},
};

static const NameTable STATE_SW[] = {
    {"examine", 7, CA_PUBLIC, STATE_COMMAND_EXAMINE},
    {"set", 3, CA_PUBLIC, STATE_COMMAND_SET},
    {"wipe", 4, CA_PUBLIC, STATE_COMMAND_WIPE},
    {"copy", 4, CA_PUBLIC, STATE_COMMAND_COPY},
    {"move", 4, CA_PUBLIC, STATE_COMMAND_MOVE},
    {nullptr, 0, 0, 0},
};

static const NameTable HELP_SW[] = {
    {"reload", 6, CA_PUBLIC, HELP_COMMAND_RELOAD},
    {nullptr, 0, 0, 0},
};

static const NameTable CLONE_SW[] = {
    {"inventory", 3, CA_PUBLIC, CLONE_INVENTORY},
    {"location", 1, CA_PUBLIC, CLONE_LOCATION},
    {nullptr, 0, 0, 0}};

static const NameTable DESTROY_SW[] = {
    {"override", 8, CA_PUBLIC, DEST_OVERRIDE},
    {"recursive", 9, CA_WIZARD, DEST_RECURSIVE},
    {nullptr, 0, 0, 0}};

static const NameTable DIG_SW[] = {{"teleport", 1, CA_PUBLIC, DIG_TELEPORT},
                                   {nullptr, 0, 0, 0}};

static const NameTable DROP_SW[] = {{"quiet", 1, CA_PUBLIC, DROP_QUIET},
                                    {nullptr, 0, 0, 0}};

static const NameTable DUMP_SW[] = {
    {"structure", 1, CA_WIZARD, DUMP_STRUCT | SW_MULTIPLE},
    {"text", 1, CA_WIZARD, DUMP_TEXT | SW_MULTIPLE},
    {"optimize", 1, CA_WIZARD, DUMP_OPTIMIZE | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

static const NameTable EMIT_SW[] = {
    {"here", 1, CA_PUBLIC, SAY_HERE | SW_MULTIPLE},
    {"room", 1, CA_PUBLIC, SAY_ROOM | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

static const NameTable ENTER_SW[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET},
                                     {nullptr, 0, 0, 0}};

static const NameTable EXAMINE_SW[] = {{"brief", 1, CA_PUBLIC, EXAM_BRIEF},
                                       {"debug", 1, CA_WIZARD, EXAM_DEBUG},
                                       {nullptr, 0, 0, 0}};

static const NameTable FEMIT_SW[] = {
    {"here", 1, CA_PUBLIC, PEMIT_HERE | SW_MULTIPLE},
    {"room", 1, CA_PUBLIC, PEMIT_ROOM | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

static const NameTable FPOSE_SW[] = {{"default", 1, CA_PUBLIC, 0},
                                     {"nospace", 1, CA_PUBLIC, SAY_NOSPACE},
                                     {nullptr, 0, 0, 0}};

static const NameTable GET_SW[] = {{"quiet", 1, CA_PUBLIC, GET_QUIET},
                                   {nullptr, 0, 0, 0}};

static const NameTable GIVE_SW[] = {{"quiet", 1, CA_WIZARD, GIVE_QUIET},
                                    {nullptr, 0, 0, 0}};

static const NameTable GOTO_SW[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET},
                                    {nullptr, 0, 0, 0}};

static const NameTable HALT_SW[] = {{"all", 1, CA_PUBLIC, HALT_ALL},
                                    {nullptr, 0, 0, 0}};

static const NameTable LEAVE_SW[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET},
                                     {nullptr, 0, 0, 0}};

static const NameTable LOOK_SW[] = {{"outside", 1, CA_PUBLIC, LOOK_OUTSIDE},
                                    {nullptr, 0, 0, 0}};

static const NameTable OPEN_SW[] = {{"inventory", 1, CA_PUBLIC, OPEN_INVENTORY},
                                    {"location", 1, CA_PUBLIC, OPEN_LOCATION},
                                    {nullptr, 0, 0, 0}};

static const NameTable PEMIT_SW[] = {
    {"contents", 1, CA_PUBLIC, PEMIT_CONTENTS | SW_MULTIPLE},
    {"object", 1, CA_PUBLIC, 0},
    {"silent", 1, CA_PUBLIC, 0},
    {"list", 1, CA_PUBLIC, PEMIT_LIST | SW_MULTIPLE},
    {nullptr, 0, 0, 0}};

static const NameTable POSE_SW[] = {{"default", 1, CA_PUBLIC, 0},
                                    {"nospace", 1, CA_PUBLIC, SAY_NOSPACE},
                                    {nullptr, 0, 0, 0}};

static const NameTable TELEPORT_SW[] = {
    {"loud", 1, CA_PUBLIC, TELEPORT_DEFAULT},
    {"quiet", 1, CA_PUBLIC, TELEPORT_QUIET},
    {nullptr, 0, 0, 0}};

static const NameTable WALL_SW[] = {
    {"emit", 1, CA_WIZARD, SAY_WALLEMIT},
    {"no_prefix", 1, CA_WIZARD, SAY_NOTAG | SW_MULTIPLE},
    {"pose", 1, CA_WIZARD, SAY_WALLPOSE},
    {"wizard", 1, CA_WIZARD, SAY_WIZSHOUT | SW_MULTIPLE},
    {"admin", 1, CA_ADMIN, SAY_ADMINSHOUT},
    {nullptr, 0, 0, 0}};

/* ---------------------------------------------------------------------------
 * Command table: Definitions for builtin commands, used to build the command
 * hash table.
 *
 * Format:  Name		Switches	Permissions Needed
 *	Key (if any)	Calling Seq			Handler
 */

static const CommandDefinition COMMAND_TABLE[] = {
    {"@admin", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_admin}},
    {"@alias", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_alias}},
    {"@boot", BOOT_SW, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_boot}},
    {"@btech", BTECH_SW, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_btech}},
    {"@chan", CHAN_SW, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_chan}},
    {"@chzone", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_chzone}},
    {"@clone",
     CLONE_SW,
     CA_WIZARD | CA_CONTENTS,
     0,
     CS_TWO_ARG,
     {.invoke = do_clone}},
    {"@create",
     nullptr,
     CA_WIZARD | CA_CONTENTS,
     0,
     CS_ONE_ARG,
     {.invoke = do_create}},
    {"@dbck", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_dbck}},
    {"@description",
     nullptr,
     CA_WIZARD,
     DESCRIPTION_EXTERNAL,
     CS_TWO_ARG,
     {.invoke = do_description}},
    {"@destroy",
     DESTROY_SW,
     CA_WIZARD,
     DEST_ONE,
     CS_ONE_ARG,
     {.invoke = do_destroy}},
    {"@dig", DIG_SW, CA_WIZARD, 0, CS_TWO_ARG | CS_ARGV, {.invoke = do_dig}},
    {"@disable",
     nullptr,
     CA_WIZARD,
     GLOB_DISABLE,
     CS_ONE_ARG,
     {.invoke = do_global}},
    {"@dump", DUMP_SW, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_dump}},
    {"@emit",
     EMIT_SW,
     CA_WIZARD | CA_LOCATION,
     SAY_EMIT,
     CS_ONE_ARG,
     {.invoke = do_say}},
    {"@enable",
     nullptr,
     CA_WIZARD,
     GLOB_ENABLE,
     CS_ONE_ARG,
     {.invoke = do_global}},
    {"@entrances", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_entrances}},
    {"@femit",
     FEMIT_SW,
     CA_WIZARD | CA_LOCATION,
     PEMIT_FEMIT,
     CS_TWO_ARG,
     {.invoke = do_pemit}},
    {"@find", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_find}},
    /*{"@fnd", NULL, 0, 0, CS_ONE_ARG|CS_UNPARSE, {.invoke =
       do_fnd_command_adapter}}, */
    {"@force",
     nullptr,
     CA_WIZARD | CA_QUEUE,
     FRC_COMMAND,
     CS_TWO_ARG | CS_NO_MACRO,
     {.invoke = do_force}},
    {"@fpose",
     FPOSE_SW,
     CA_WIZARD | CA_LOCATION,
     PEMIT_FPOSE,
     CS_TWO_ARG,
     {.invoke = do_pemit}},
    {"@fsay",
     nullptr,
     CA_WIZARD | CA_LOCATION,
     PEMIT_FSAY,
     CS_TWO_ARG,
     {.invoke = do_pemit}},
    {"@halt", HALT_SW, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_halt}},
    {"@help", HELP_SW, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_help_admin}},
    {"@internal-description",
     nullptr,
     CA_WIZARD,
     DESCRIPTION_INTERNAL,
     CS_TWO_ARG,
     {.invoke = do_description}},
    {"@last", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_last}},
    {"@link", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_link}},
    {"@lua", LUA_SW, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_lua}},
    {"@list", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_list}},
    {"@log", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_log}},
    {"@mech",
     MECH_ADMIN_SW,
     CA_WIZARD,
     0,
     CS_ONE_ARG,
     {.invoke = do_mech_admin}},
    {"@name", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_name}},
    {"@newpassword",
     nullptr,
     CA_WIZARD,
     PASS_ANY,
     CS_TWO_ARG,
     {.invoke = do_newpassword}},
    {"@oemit",
     nullptr,
     CA_WIZARD | CA_LOCATION,
     PEMIT_OEMIT,
     CS_TWO_ARG,
     {.invoke = do_pemit}},
    {"@open", OPEN_SW, CA_WIZARD, 0, CS_TWO_ARG | CS_ARGV, {.invoke = do_open}},
    {"@pcreate",
     nullptr,
     CA_WIZARD,
     PCRE_PLAYER,
     CS_TWO_ARG,
     {.invoke = do_pcreate}},
    {"@pemit",
     PEMIT_SW,
     CA_WIZARD,
     PEMIT_PEMIT,
     CS_TWO_ARG,
     {.invoke = do_pemit}},
    {"@npemit",
     PEMIT_SW,
     CA_WIZARD,
     PEMIT_PEMIT,
     CS_TWO_ARG | CS_UNPARSE,
     {.invoke = do_pemit}},
    {"@power", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_power}},
    {"@readcache", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_readcache}},
    {"@search",
     nullptr,
     CA_WIZARD,
     SRCH_SEARCH,
     CS_ONE_ARG,
     {.invoke = do_search}},
    {"@flag", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_flag}},
    {"@shutdown", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_shutdown}},
    {"@state", STATE_SW, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_state}},
    {"@stats", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_stats}},
    {"@teleport",
     TELEPORT_SW,
     CA_WIZARD,
     TELEPORT_DEFAULT,
     CS_TWO_ARG,
     {.invoke = do_teleport}},
    {"@unlink", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_unlink}},
    {"@wait",
     nullptr,
     CA_WIZARD | CA_QUEUE,
     0,
     CS_TWO_ARG | CS_STRIP_AROUND | CS_NO_MACRO,
     {.invoke = do_wait}},
    {"@wall", WALL_SW, CA_WIZARD, SAY_SHOUT, CS_ONE_ARG, {.invoke = do_say}},
    {"@session", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_session}},
    {"@telnet", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_telnet}},
    {"@who", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_who}},
    {"addcom", nullptr, CA_NO_IC, 0, CS_TWO_ARG, {.invoke = do_addcom}},
    {"allcom", nullptr, CA_NO_IC, 0, CS_ONE_ARG, {.invoke = do_allcom}},
    {"comlist", nullptr, CA_NO_IC, 0, CS_NO_ARGS, {.invoke = do_comlist}},
    {"clearcom", nullptr, CA_NO_IC, 0, CS_NO_ARGS, {.invoke = do_clearcom}},
    {"color", nullptr, 0, 0, CS_ONE_ARG, {.invoke = do_color}},
    {"delcom", nullptr, CA_NO_IC, 0, CS_ONE_ARG, {.invoke = do_delcom}},
    {"drop",
     DROP_SW,
     CA_CONTENTS | CA_LOCATION,
     0,
     CS_ONE_ARG,
     {.invoke = do_drop}},
    {"enter", ENTER_SW, CA_LOCATION, 0, CS_ONE_ARG, {.invoke = do_enter}},
    {"@examine", EXAMINE_SW, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_examine}},
    {"get", GET_SW, CA_LOCATION, 0, CS_ONE_ARG, {.invoke = do_get}},
    {"give", GIVE_SW, CA_LOCATION, 0, CS_TWO_ARG, {.invoke = do_give}},
    {"goto", GOTO_SW, CA_LOCATION, 0, CS_ONE_ARG, {.invoke = do_move}},
    {"help", nullptr, 0, 0, CS_ONE_ARG, {.invoke = do_help}},
    {"inventory",
     nullptr,
     0,
     LOOK_INVENTORY,
     CS_NO_ARGS,
     {.invoke = do_inventory}},
    {"leave", LEAVE_SW, CA_LOCATION, 0, CS_NO_ARGS, {.invoke = do_leave}},
    {"look", LOOK_SW, CA_LOCATION, LOOK_LOOK, CS_ONE_ARG, {.invoke = do_look}},
    {"page", nullptr, 0, 0, CS_TWO_ARG, {.invoke = do_page}},
    {"pose", POSE_SW, CA_LOCATION, SAY_POSE, CS_ONE_ARG, {.invoke = do_say}},
    {"quit", nullptr, 0, 0, CS_NO_ARGS, {.invoke = do_quit}},
    {"say", nullptr, CA_LOCATION, SAY_SAY, CS_ONE_ARG, {.invoke = do_say}},
    {"use", nullptr, 0, 0, CS_ONE_ARG, {.invoke = do_use}},
    {"version", nullptr, 0, 0, CS_NO_ARGS, {.invoke = do_version}},
    {"+show", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_show}},
    {"+rolls", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_show_stat}},
    {"+charclear", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_charclear}},
    {"\\",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG,
     {.invoke = do_say}},
    {"#",
     nullptr,
     CA_QUEUE | CF_DARK,
     0,
     CS_ONE_ARG,
     {.invoke = do_force_prefixed}},
    {":",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG | CS_LEADIN,
     {.invoke = do_say}},
    {";",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG | CS_LEADIN,
     {.invoke = do_say}},
    {"\"",
     nullptr,
     CA_LOCATION | CF_DARK,
     SAY_PREFIX,
     CS_ONE_ARG | CS_LEADIN,
     {.invoke = do_say}},
    {nullptr, nullptr, 0, 0, 0, {}}};

bool command_builtin_catalog_install(CommandRegistry *registry) {
  return command_catalog_install(
      registry, COMMAND_TABLE,
      (sizeof(COMMAND_TABLE) / sizeof(COMMAND_TABLE[0])) - 1);
}

CMDENT *command_prefix_entry_at(const CommandRegistry *registry, size_t index) {
  return *(CMDENT *const *)checked_storage_at_const(
      (const void *)registry->prefix_commands,
      sizeof(registry->prefix_commands) / sizeof(*registry->prefix_commands),
      sizeof(*registry->prefix_commands), index);
}

/*
 * Returns 1 if player is in an IC location, 0 if not. Take into account
 * the ooc_comsys directive.
 */
