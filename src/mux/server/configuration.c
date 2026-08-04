/* configuration.c - Configuration parsing and defaults */

#include "mux/server/configuration.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech_context.h"
#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/macro.h"
#include "mux/network/connection_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/configuration_internal.h"
#include "mux/server/configuration_toml.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
/* default (runtime-resettable) cache parameters */

constexpr int CACHE_DEPTH = 10;
constexpr int CACHE_WIDTH = 20;

ServerConfiguration *server_configuration_create(void) {
  return calloc(1, sizeof(ServerConfiguration));
}

void server_configuration_destroy(ServerConfiguration *configuration) {
  free(configuration);
}

/*
 * ---------------------------------------------------------------------------
 * * CONFPARM: Data used to find fields in ServerConfiguration.
 */

static void *configuration_resolve_location(ConfigurationContext *context,
                                            const CONF *entry) {
  uintptr_t location = (uintptr_t)entry->loc;

  if (location > 0 && location <= sizeof(ServerConfiguration))
    return (char *)context->configuration + location - 1;
  if (location > sizeof(ServerConfiguration) &&
      location <= sizeof(ServerConfiguration) + sizeof(CommandRegistry))
    return (char *)context->command_registry + location -
           sizeof(ServerConfiguration) - 1;
  if (location > sizeof(ServerConfiguration) + sizeof(CommandRegistry) &&
      location <= sizeof(ServerConfiguration) + sizeof(CommandRegistry) +
                      sizeof(WorldIndexes))
    return (char *)context->world_indexes + location -
           sizeof(ServerConfiguration) - sizeof(CommandRegistry) - 1;
  if (location > sizeof(ServerConfiguration) + sizeof(CommandRegistry) +
                     sizeof(WorldIndexes) &&
      location <= sizeof(ServerConfiguration) + sizeof(CommandRegistry) +
                      sizeof(WorldIndexes) + sizeof(AccessControlStore))
    return (char *)context->world->access_control + location -
           sizeof(ServerConfiguration) - sizeof(CommandRegistry) -
           sizeof(WorldIndexes) - 1;
  return entry->loc;
}

/*
 * ---------------------------------------------------------------------------
 * * External symbols.
 */

extern NameTable logdata_nametab[];
extern NameTable logoptions_nametab[];
extern NameTable access_nametab[];
extern NameTable list_names[];
extern CONF conftable[];

/*
 * ---------------------------------------------------------------------------
 * * configuration_initialize: Initialize context configuration defaults.
 */

void configuration_initialize(ConfigurationContext *context) {
  StringCopy(context->configuration->database.gamedb, "");
  StringCopy(context->configuration->database.mech_db, "mechs");
  StringCopy(context->configuration->database.map_db, "maps");
  context->configuration->btech_explode_reactor = 1;
  context->configuration->btech_explode_time = 120;
  context->configuration->btech_explode_ammo = 1;
  context->configuration->btech_explode_stop = 0;
  context->configuration->btech_stackpole = 1;
  context->configuration->btech_phys_use_pskill = 1;
  context->configuration->btech_erange = 1;
  context->configuration->btech_hit_arcs = 0;
  context->configuration->btech_dig_only_fs = 0;
  context->configuration->btech_digbonus = 3;
  context->configuration->btech_vcrit = 2;
  context->configuration->btech_dynspeed = 1;
  context->configuration->btech_ic = 1;
  context->configuration->btech_parts = 1;
  context->configuration->btech_slowdown = 2;
  context->configuration->btech_fasaturn = 1;
  context->configuration->btech_fasacrit = 0;
  context->configuration->btech_fasaadvvtolcrit = 0;
  context->configuration->btech_fasaadvvhlcrit = 0;
  context->configuration->btech_fasaadvvhlfire = 0;
  context->configuration->btech_divrotordamage = 0;
  context->configuration->btech_moddamagewithrange = 0;
  context->configuration->btech_moddamagewithwoods = 0;
  context->configuration->btech_hotloadaddshalfbthmod = 0;
  context->configuration->btech_nofusionvtolfuel = 0;
  context->configuration->btech_tankfriendly = 0;
  context->configuration->btech_newterrain = 0;
  context->configuration->btech_skidcliff = 0;
  context->configuration->btech_xp_bthmod = 0;
  context->configuration->btech_xp_missilemod = 100;
  context->configuration->btech_xp_ammomod = 100;
  context->configuration->btech_defaultweapdam = 5;
  context->configuration->btech_xp_modifier = 100;
  context->configuration->btech_defaultweapbv = 120;
  context->configuration->btech_xp_usePilotBVMod = 1;
  context->configuration->btech_oldxpsystem = 1;
  context->configuration->btech_xp_vrtmod = 0;
  context->configuration->btech_limitedrepairs = 0;
  context->configuration->btech_newcharge = 0;
  context->configuration->btech_tl3_charge = 0;
  context->configuration->btech_xploss = 666;
  context->configuration->btech_critlevel = 100;
  context->configuration->btech_tankshield = 0;
  context->configuration->btech_newstagger = 1;
  context->configuration->btech_newstaggertons = 1;
  context->configuration->btech_newstaggertime = 5;
  context->configuration->btech_extendedmovemod = 1;
  context->configuration->btech_stacking = 2;
  context->configuration->btech_stackdamage = 100;
  context->configuration->btech_mw_losmap = 1;
  context->configuration->btech_seismic_see_stopped = 0;
  context->configuration->btech_exile_stun_code = 0;
  context->configuration->btech_roll_on_backwalk = 1;
  context->configuration->btech_usedmechstore = 0;
  context->configuration->btech_ooc_comsys = 0;
  context->configuration->btech_idf_requires_spotter = 1;
  context->configuration->btech_vtol_ice_causes_fire = 1;
  context->configuration->btech_glancing_blows = 1;
  context->configuration->btech_inferno_penalty = 0;
  context->configuration->btech_perunit_xpmod = 1;
  context->configuration->btech_tsm_tow_bonus = 1;
  context->configuration->btech_tsm_sprint_bonus = 1;
  context->configuration->btech_heatcutoff = 1;
  context->configuration->btech_sprint_bth = -4;
  context->configuration->btech_cost_debug = 0;
  context->configuration->btech_noisy_xpgain = 0;
  context->configuration->btech_xpgain_cap = 10;
  context->configuration->btech_transported_unit_death = 1;
  context->configuration->btech_mwpickup_action = 1;
  context->configuration->btech_standcareful = 1;
  context->configuration->btech_maxtechtime = 600;
  context->configuration->btech_blzmapmode = 0;
  context->configuration->btech_extended_piloting = 1;
  context->configuration->btech_extended_gunnery = 1;
  context->configuration->btech_xploss_for_mw = 1;
  context->configuration->btech_variable_techtime = 0;
  context->configuration->btech_techtime_mod = 0;
  context->configuration->btech_statengine_obj = -1;
#ifdef BT_FREETECHTIME
  context->configuration->btech_freetechtime = 0;
#endif
#ifdef BT_COMPLEXREPAIRS
  context->configuration->btech_complexrepair = 1;
#endif
  context->configuration->allow_chanlurking = 0;
  context->configuration->afterlife_dbref = 220;
  context->configuration->port = 6250;
  context->configuration->init_size = 1000;
  StringCopy(context->configuration->conn_file, "text/connect.txt");
  StringCopy(context->configuration->conn_dir, "");
  StringCopy(context->configuration->quit_file, "text/quit.txt");
  StringCopy(context->configuration->down_file, "text/down.txt");
  StringCopy(context->configuration->full_file, "text/full.txt");
  StringCopy(context->configuration->site_file, "text/badsite.txt");
  StringCopy(context->configuration->help_dir, "help");
  StringCopy(context->configuration->down_msg, "");
  StringCopy(context->configuration->full_msg, "");
  StringCopy(context->configuration->database.dump_msg, "");
  StringCopy(context->configuration->database.postdump_msg, "");
  StringCopy(context->configuration->public_channel, "Public");
  context->configuration->name_spaces = 1;
  context->configuration->database.fork_dump = 1;
  context->configuration->max_players = -1;
  context->configuration->database.dump_interval = 3600;
  context->configuration->check_interval = 600;
  context->configuration->dump_offset = 0;
  context->configuration->check_offset = 300;
  context->configuration->idle_timeout = 3600;
  context->configuration->conn_timeout = 120;
  context->configuration->idle_interval = 60;
  context->configuration->retry_limit = 3;
  context->configuration->player_password_length_limit = 64;
  context->configuration->password_hash_opslimit = 3;
  context->configuration->password_hash_memlimit = 12 * 1024 * 1024;
  context->configuration->login_attempt_burst = 3;
  context->configuration->login_attempt_refill = 10;
  context->configuration->login_hash_limit = 5;
  context->configuration->output_limit = 16384;
  context->configuration->command_queue_limit = 100;
  context->configuration->command_queue_idle_chunk = 10;
  context->configuration->command_queue_active_chunk = 10;
  /*
   * -- ??? Running SC on a non-SC DB may cause problems
   */
  context->configuration->space_compress = 1;
  context->configuration->start_room = 0;
  context->configuration->start_home = -1;
  context->configuration->default_home = -1;
  StringCopy(context->configuration->default_thing_lua_parent, "");
  StringCopy(context->configuration->default_room_lua_parent, "");
  StringCopy(context->configuration->default_exit_lua_parent, "");
  StringCopy(context->configuration->default_player_lua_parent, "");
  context->configuration->default_player_flags = (ObjectFlagSet){0};
  context->configuration->default_room_flags = (ObjectFlagSet){0};
  context->configuration->default_exit_flags = (ObjectFlagSet){0};
  context->configuration->default_thing_flags = (ObjectFlagSet){0};
  StringCopy(context->configuration->mud_name, "TinyMUX");
  context->configuration->command_quota_interval = 100;
  context->configuration->command_quota_max = 100;
  context->configuration->command_quota_increment = 5;
  context->configuration->is_login_enabled = true;
  context->configuration->is_command_queue_enabled = true;
  context->configuration->is_checkpointing_enabled = true;
  context->configuration->is_db_check_enabled = true;
  context->configuration->is_idle_check_enabled = true;
  context->configuration->log_options =
      LOG_ALWAYS | LOG_BUGS | LOG_SECURITY | LOG_NET | LOG_LOGIN | LOG_DBSAVES |
      LOG_CONFIGMODS | LOG_SHOUTS | LOG_STARTUP | LOG_WIZARD | LOG_PROBLEMS |
      LOG_PCREATES;
  context->configuration->log_info = LOGOPT_TIMESTAMP | LOGOPT_LOC;
  context->configuration->ntfy_nest_lim = 20;
  context->configuration->stack_limit = 50;
  context->configuration->cache_trim = 0;
  context->configuration->cache_depth = CACHE_DEPTH;
  context->configuration->cache_width = CACHE_WIDTH;
  context->configuration->cache_names = 1;
  StringCopy(context->configuration->lua.directory, "lua");
  context->configuration->lua.memory_limit = 64 * 1024 * 1024;
  context->configuration->lua.state_value_limit = 64 * 1024;
  context->configuration->lua.state_entry_limit = 1024;
  context->configuration->lua.state_object_limit = 1024 * 1024;

  context->configuration->player_zone = 0;
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_log_not_found: Log a 'parameter not found' error.
 */
void configuration_log_not_found(ConfigurationContext *context, DbRef player,
                                 const char *cmd, const char *thingname,
                                 const char *thing) {
  char *buff;

  if (context->configuration->is_initializing) {
    log_error(context->log, LOG_STARTUP, "CNF", "NFND", "%s: %s %s not found.",
              cmd, thingname, thing);
  } else {
    buff = alloc_lbuf("configuration_log_not_found");
    snprintf(buff, LBUF_SIZE, "%s %s not found", thingname, thing);
    notify_checked(&context->command->evaluation, player, player, buff,
                   MSG_ME_ALL | MSG_F_DOWN);
    free_lbuf(buff);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_log_syntax: Log a syntax error.
 */

void configuration_log_syntax(ConfigurationContext *context, DbRef player,
                              const char *cmd, const char *template,
                              const char *arg) {
  if (context->configuration->is_initializing) {
    log_error(context->log, LOG_STARTUP, "CNF", "SYNTX", "%s: %s %s", cmd,
              template, arg);
  } else {
    notify_printf(&context->command->evaluation, player, "%s%s", template, arg);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * cf_status_from_succfail: Return command status from succ and fail info
 */

int configuration_status_from_succfail(DbRef player, char *cmd, int success,
                                       int failure,
                                       ConfigurationContext *context) {

  /*
   * If any successes, return SUCCESS(0) if no failures or * * * * *
   * PARTIAL_SUCCESS(1) if any failures.
   */

  if (success > 0)
    return ((failure == 0) ? 0 : 1);

  /*
   * No successes.  If no failures indicate nothing done. Always return
   *
   * *  * *  * *  * *  * * FAILURE(-1)
   */

  if (failure == 0) {
    if (context->configuration->is_initializing) {
      log_error(context->log, LOG_STARTUP, "CNF", "NDATA", "%s: Nothing to set",
                cmd);
    } else {
      notify_checked(&context->command->evaluation, player, player,
                     "Nothing to set", MSG_ME_ALL | MSG_F_DOWN);
    }
  }
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_int: Set integer parameter.
 */

int configuration_set(ConfigurationContext *context, char *cp, char *ap,
                      DbRef player) {
  CONF *tp;
  int i;
  char *buff = nullptr;

  /*
   * Search the config parameter table for the command. If we find it,
   * call the handler to parse the argument.
   */

  for (tp = conftable; tp->pname; tp++) {
    if (!strcmp(tp->pname, cp)) {
      if (!context->configuration->is_initializing &&
          !check_access(context->database, context->configuration, player,
                        tp->flags)) {
        notify_checked(&context->command->evaluation, player, player,
                       "Permission denied.", MSG_ME_ALL | MSG_F_DOWN);
        return (-1);
      }
      buff = alloc_lbuf("configuration_set");
      StringCopy(buff, ap);
      i = tp->interpreter(configuration_resolve_location(context, tp), ap,
                          tp->extra, player, cp, context);
      if (!context->configuration->is_initializing) {
        log_error(context->log, LOG_CONFIGMODS, "CFG", "UPDAT",
                  "%s entered config directive: %s with args '%s'. Status: %s",
                  game_object_name(context->database, player), cp, buff,
                  (i == 0 ? "Success"
                          : (i == 1 ? "Partial success"
                                    : (i == -1 ? "Failure" : "Strange"))));
      }
      free_lbuf(buff);
      return i;
    }
  }

  /*
   * Config directive not found.  Complain about it.
   */

  configuration_log_not_found(context, player, "Set", "Config directive", cp);
  return (-1);
}

/*
 * ---------------------------------------------------------------------------
 * * do_admin: Command handler to set config params at runtime
 */
void do_admin(CommandInvocation *invocation) {
  int i;

  i = configuration_set(invocation->context->runtime->configuration_context,
                        invocation->first, invocation->second,
                        invocation->player);
  if ((i >= 0) &&
      !is_quiet(invocation->context->world->database, invocation->player))
    notify_checked(&invocation->context->evaluation, invocation->player,
                   invocation->player, "Set.", MSG_ME_ALL | MSG_F_DOWN);
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_toml_dispatch_to_set: Adapter from the TOML loader's
 * dispatch callback into configuration_set().
 */
static int configuration_toml_dispatch_to_set(const char *pname,
                                              const char *args, void *ctx) {
  ConfigurationContext *context = ctx;
  /* configuration_set()'s (char *, char *) signature isn't const-correct;
     it only reads these strings. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  return configuration_set(context, (char *)pname, (char *)args, 0);
#pragma clang diagnostic pop
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_read: Read in config parameters from named file
 */
int configuration_read(ConfigurationContext *context, char *fn) {
  char errbuf[256];
  bool ok;

  StringCopy(context->configuration->config_file, fn);
  context->fatal_error = false;
  context->configuration->is_initializing = true;
  ok = configuration_toml_load(fn, configuration_toml_dispatch_to_set, context,
                               errbuf, sizeof(errbuf));
  context->configuration->is_initializing = false;
  if (context->fatal_error && errbuf[0] == '\0')
    snprintf(errbuf, sizeof(errbuf), "invalid styled-text configuration");
  if (!ok || context->fatal_error) {
    fprintf(stderr, "Error reading config file '%s': %s\n", fn, errbuf);
    return -1;
  }
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_list_access: List access to config directives.
 */
void configuration_list_access(EvaluationContext *evaluation, DbRef player) {
  CONF *tp;
  char buff[MBUF_SIZE];

  for (tp = conftable; tp->pname; tp++) {
    if (is_god(evaluation->world->database, player) ||
        check_access(evaluation->world->database,
                     evaluation->world->configuration, player, tp->flags)) {
      snprintf(buff, MBUF_SIZE, "%s:", tp->pname);
      name_table_list_set(evaluation, evaluation->world->configuration, player,
                          access_nametab, tp->flags, buff, 1);
    }
  }
}
