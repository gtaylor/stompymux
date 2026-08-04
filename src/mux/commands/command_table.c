/*
 * command.c - command parser and support routines
 */

#include <stdlib.h>
#include <strings.h>

#include "btech_context.h" // IWYU pragma: keep
#include "btmux_build_config.h"
#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_internal.h"
#include "mux/commands/command_keys.h"
#include "mux/commands/look.h"
#include "mux/commands/macro.h" // IWYU pragma: keep
#include "mux/communication/comsys.h"
#include "mux/communication/speech.h"
#include "mux/help/help_command.h"
#include "mux/objects/attrs.h"
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
#include "mux/world/inventory_commands.h"
#include "mux/world/movement_commands.h"

/*
 * ---------------------------------------------------------------------------
 * * Switch tables for the various commands.
 */

/*
 * (typically via a switch alias)
 */

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
    {"viewparent", 10, CA_PUBLIC, LUA_COMMAND_VIEWPARENT},
    {nullptr, 0, 0, 0},
};

NameTable state_sw[] = {
    {"examine", 7, CA_PUBLIC, STATE_COMMAND_EXAMINE},
    {"set", 3, CA_PUBLIC, STATE_COMMAND_SET},
    {"wipe", 4, CA_PUBLIC, STATE_COMMAND_WIPE},
    {"copy", 4, CA_PUBLIC, STATE_COMMAND_COPY},
    {"move", 4, CA_PUBLIC, STATE_COMMAND_MOVE},
    {nullptr, 0, 0, 0},
};

NameTable help_sw[] = {
    {"reload", 6, CA_PUBLIC, HELP_COMMAND_RELOAD},
    {nullptr, 0, 0, 0},
};

NameTable clone_sw[] = {{"inventory", 3, CA_PUBLIC, CLONE_INVENTORY},
                        {"location", 1, CA_PUBLIC, CLONE_LOCATION},
                        {nullptr, 0, 0, 0}};

NameTable destroy_sw[] = {{"override", 8, CA_PUBLIC, DEST_OVERRIDE},
                          {"recursive", 9, CA_WIZARD, DEST_RECURSIVE},
                          {nullptr, 0, 0, 0}};

NameTable dig_sw[] = {{"teleport", 1, CA_PUBLIC, DIG_TELEPORT},
                      {nullptr, 0, 0, 0}};

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
                          {nullptr, 0, 0, 0}};

NameTable femit_sw[] = {{"here", 1, CA_PUBLIC, PEMIT_HERE | SW_MULTIPLE},
                        {"room", 1, CA_PUBLIC, PEMIT_ROOM | SW_MULTIPLE},
                        {nullptr, 0, 0, 0}};

NameTable fpose_sw[] = {{"default", 1, CA_PUBLIC, 0},
                        {"nospace", 1, CA_PUBLIC, SAY_NOSPACE},
                        {nullptr, 0, 0, 0}};

NameTable get_sw[] = {{"quiet", 1, CA_PUBLIC, GET_QUIET}, {nullptr, 0, 0, 0}};

NameTable give_sw[] = {{"quiet", 1, CA_WIZARD, GIVE_QUIET}, {nullptr, 0, 0, 0}};

NameTable goto_sw[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET}, {nullptr, 0, 0, 0}};

NameTable halt_sw[] = {{"all", 1, CA_PUBLIC, HALT_ALL}, {nullptr, 0, 0, 0}};

NameTable leave_sw[] = {{"quiet", 1, CA_PUBLIC, MOVE_QUIET},
                        {nullptr, 0, 0, 0}};

NameTable look_sw[] = {{"outside", 1, CA_PUBLIC, LOOK_OUTSIDE},
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

NameTable set_sw[] = {{"quiet", 1, CA_PUBLIC, SET_QUIET}, {nullptr, 0, 0, 0}};

NameTable teleport_sw[] = {{"loud", 1, CA_PUBLIC, TELEPORT_DEFAULT},
                           {"quiet", 1, CA_PUBLIC, TELEPORT_QUIET},
                           {nullptr, 0, 0, 0}};

NameTable wall_sw[] = {{"emit", 1, CA_WIZARD, SAY_WALLEMIT},
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

CMDENT command_table[] = {
    {"@admin", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_admin}},
    {"@alias", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_alias}},
    {"@boot", boot_sw, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_boot}},
    {"@chan", chan_sw, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_chan}},
    {"@chzone", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_chzone}},
    {"@clone",
     clone_sw,
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
    {"@destroy",
     destroy_sw,
     CA_WIZARD,
     DEST_ONE,
     CS_ONE_ARG,
     {.invoke = do_destroy}},
    {"@desc", nullptr, CA_WIZARD, A_DESC, CS_TWO_ARG, {.invoke = do_setattr}},
    {"@idesc", nullptr, CA_WIZARD, A_IDESC, CS_TWO_ARG, {.invoke = do_setattr}},
    {"@dig", dig_sw, CA_WIZARD, 0, CS_TWO_ARG | CS_ARGV, {.invoke = do_dig}},
    {"@disable",
     nullptr,
     CA_WIZARD,
     GLOB_DISABLE,
     CS_ONE_ARG,
     {.invoke = do_global}},
    {"@dump", dump_sw, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_dump}},
    {"@emit",
     emit_sw,
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
     femit_sw,
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
     fpose_sw,
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
    {"@halt", halt_sw, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_halt}},
    {"@help", help_sw, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_help_admin}},
    {"@last", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_last}},
    {"@link", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_link}},
    {"@lua", lua_sw, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_lua}},
    {"@list", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_list}},
#ifdef ARBITRARY_LOGFILES
    {"@log", nullptr, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_log}},
#endif
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
    {"@open", open_sw, CA_WIZARD, 0, CS_TWO_ARG | CS_ARGV, {.invoke = do_open}},
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
     CS_TWO_ARG,
     {.invoke = do_pemit}},
    {"@npemit",
     pemit_sw,
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
    {"@set", set_sw, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_set}},
    {"@shutdown", nullptr, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_shutdown}},
    {"@state", state_sw, CA_WIZARD, 0, CS_TWO_ARG, {.invoke = do_state}},
    {"@stats", nullptr, CA_WIZARD, 0, CS_NO_ARGS, {.invoke = do_stats}},
    {"@teleport",
     teleport_sw,
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
    {"@wall", wall_sw, CA_WIZARD, SAY_SHOUT, CS_ONE_ARG, {.invoke = do_say}},
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
     drop_sw,
     CA_CONTENTS | CA_LOCATION,
     0,
     CS_ONE_ARG,
     {.invoke = do_drop}},
    {"enter", enter_sw, CA_LOCATION, 0, CS_ONE_ARG, {.invoke = do_enter}},
    {"@examine", examine_sw, CA_WIZARD, 0, CS_ONE_ARG, {.invoke = do_examine}},
    {"get", get_sw, CA_LOCATION, 0, CS_ONE_ARG, {.invoke = do_get}},
    {"give", give_sw, CA_LOCATION, 0, CS_TWO_ARG, {.invoke = do_give}},
    {"goto", goto_sw, CA_LOCATION, 0, CS_ONE_ARG, {.invoke = do_move}},
    {"help", nullptr, 0, 0, CS_ONE_ARG, {.invoke = do_help}},
    {"inventory",
     nullptr,
     0,
     LOOK_INVENTORY,
     CS_NO_ARGS,
     {.invoke = do_inventory}},
    {"leave", leave_sw, CA_LOCATION, 0, CS_NO_ARGS, {.invoke = do_leave}},
    {"look", look_sw, CA_LOCATION, LOOK_LOOK, CS_ONE_ARG, {.invoke = do_look}},
    {"page", nullptr, 0, 0, CS_TWO_ARG, {.invoke = do_page}},
    {"pose", pose_sw, CA_LOCATION, SAY_POSE, CS_ONE_ARG, {.invoke = do_say}},
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
    {(char *)nullptr, nullptr, 0, 0, 0, {nullptr}}};

void init_cmdtab(CommandRegistry *registry) {
  CMDENT *cp;
  hash_table_initialize(&registry->commands, 250 * HASH_FACTOR);

  /*
   * Load the builtin commands
   */

  for (cp = command_table; cp->cmdname; cp++)
    hash_table_add(cp->cmdname, (int *)cp, &registry->commands);

  set_prefix_cmds(registry);

  registry->goto_command = hash_table_find("goto", &registry->commands);
}

void command_aliases_destroy(HashTable *commands) {
  CMDENT **aliases = nullptr;
  size_t alias_count = 0;

  if (commands == nullptr || commands->tree == nullptr)
    return;
  for (char *key = hash_table_first_key(commands); key != nullptr;
       key = hash_table_next_key(commands)) {
    CMDENT *command = hash_table_find(key, commands);
    bool built_in = false;

    for (CMDENT *candidate = command_table; candidate->cmdname; candidate++) {
      if (command == candidate) {
        built_in = true;
        break;
      }
    }
    if (built_in || strcasecmp(key, command->cmdname))
      continue;
    CMDENT **grown = realloc(aliases, (alias_count + 1) * sizeof(*aliases));
    if (grown == nullptr)
      break;
    aliases = grown;
    aliases[alias_count++] = command;
  }
  for (size_t index = 0; index < alias_count; index++) {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    free((void *)aliases[index]->cmdname);
#pragma clang diagnostic pop
    free(aliases[index]);
  }
  free(aliases);
}

void set_prefix_cmds(CommandRegistry *registry) {
  /*
   * Load the command prefix table.  Note - these commands can never *
   * * * * * * be typed in by a user because commands are lowercased *
   * before  * * * * the  * hash table is checked. The names are *
   * abbreviated to * * * minimise * * name checking time.
   */

  for (size_t i = 0; i < sizeof(registry->prefix_commands) /
                             sizeof(*registry->prefix_commands);
       i++)
    registry->prefix_commands[i] = nullptr;
  registry->prefix_commands['"'] = hash_table_find("\"", &registry->commands);
  registry->prefix_commands[':'] = hash_table_find(":", &registry->commands);
  registry->prefix_commands[';'] = hash_table_find(";", &registry->commands);
  registry->prefix_commands['\\'] = hash_table_find("\\", &registry->commands);
  registry->prefix_commands['#'] = hash_table_find("#", &registry->commands);
}

/*
 * Returns 1 if player is in an IC location, 0 if not. Take into account
 * the ooc_comsys directive.
 */
