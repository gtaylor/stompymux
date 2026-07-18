/* configuration.c - Configuration parsing and defaults */

#include "mux/server/configuration.h"

#include "mux/server/configuration_toml.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"

#include <arpa/inet.h>
#include <stddef.h>
#include <stdint.h>

#include "mux/commands/command.h"
#include "mux/commands/functions.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
/* default (runtime-resettable) cache parameters */

constexpr int CACHE_DEPTH = 10;
constexpr int CACHE_WIDTH = 20;

/*
 * ---------------------------------------------------------------------------
 * * CONFPARM: Data used to find fields in ServerConfiguration.
 */

typedef struct confparm CONF;
typedef int (*ConfigurationInterpreter)(void *value, char *text, long extra,
                                        DbRef player, char *command,
                                        MuxServer *server);
struct confparm {
  const char *pname;                    /* parm name */
  ConfigurationInterpreter interpreter; /* routine to interp parameter */
  int flags;                            /* control flags */
  int *loc;                             /* where to store value */
  long extra;                           /* extra data for interpreter */
};

typedef int (*ConfigurationIntInterpreter)(int *value, char *text, long extra,
                                           DbRef player, char *command,
                                           MuxServer *server);
typedef int (*ConfigurationListInterpreter)(long **value, char *text,
                                            long extra, DbRef player,
                                            char *command, MuxServer *server);

static int configuration_call_int(ConfigurationIntInterpreter interpreter,
                                  void *value, char *text, long extra,
                                  DbRef player, char *command,
                                  MuxServer *server) {
  return interpreter(value, text, extra, player, command, server);
}

static int configuration_call_list(ConfigurationListInterpreter interpreter,
                                   void *value, char *text, long extra,
                                   DbRef player, char *command,
                                   MuxServer *server) {
  return interpreter(value, text, extra, player, command, server);
}

static int configuration_call_direct(ConfigurationInterpreter interpreter,
                                     void *value, char *text, long extra,
                                     DbRef player, char *command,
                                     MuxServer *server) {
  return interpreter(value, text, extra, player, command, server);
}

#define DEFINE_CONFIGURATION_ADAPTER(function)                                 \
  static int function##_configuration_adapter(                                 \
      void *value, char *text, long extra, DbRef player, char *command,        \
      MuxServer *server) {                                                     \
    return _Generic((function),                                                \
        ConfigurationIntInterpreter: configuration_call_int,                   \
        ConfigurationListInterpreter: configuration_call_list,                 \
        ConfigurationInterpreter: configuration_call_direct)(                  \
        function, value, text, extra, player, command, server);                \
  }

#define CONFIG_LOC(member)                                                     \
  ((int *)(uintptr_t)(offsetof(ServerConfiguration, member) + 1))
#define COMMAND_LOC(member)                                                    \
  ((int *)(uintptr_t)(sizeof(ServerConfiguration) +                            \
                      offsetof(CommandRegistry, member) + 1))
#define WORLD_LOC(member)                                                      \
  ((int *)(uintptr_t)(sizeof(ServerConfiguration) + sizeof(CommandRegistry) +  \
                      offsetof(WorldIndexes, member) + 1))
#define ACCESS_LOC(member)                                                     \
  ((int *)(uintptr_t)(sizeof(ServerConfiguration) + sizeof(CommandRegistry) +  \
                      sizeof(WorldIndexes) +                                   \
                      offsetof(AccessControlStore, member) + 1))

static void *configuration_resolve_location(MuxServer *server,
                                            const CONF *entry) {
  uintptr_t location = (uintptr_t)entry->loc;

  if (location > 0 && location <= sizeof(ServerConfiguration))
    return (char *)server->configuration + location - 1;
  if (location > sizeof(ServerConfiguration) &&
      location <= sizeof(ServerConfiguration) + sizeof(CommandRegistry))
    return (char *)&server->command_registry + location -
           sizeof(ServerConfiguration) - 1;
  if (location > sizeof(ServerConfiguration) + sizeof(CommandRegistry) &&
      location <= sizeof(ServerConfiguration) + sizeof(CommandRegistry) +
                      sizeof(WorldIndexes))
    return (char *)&server->world_indexes + location -
           sizeof(ServerConfiguration) - sizeof(CommandRegistry) - 1;
  if (location > sizeof(ServerConfiguration) + sizeof(CommandRegistry) +
                     sizeof(WorldIndexes) &&
      location <= sizeof(ServerConfiguration) + sizeof(CommandRegistry) +
                      sizeof(WorldIndexes) + sizeof(AccessControlStore))
    return (char *)&server->access_control + location -
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
extern NameTable attraccess_nametab[];
extern NameTable list_names[];
extern CONF conftable[];

/*
 * ---------------------------------------------------------------------------
 * * configuration_initialize: Initialize server configuration defaults.
 */

void configuration_initialize(MuxServer *server) {
  StringCopy(server->configuration->database.gamedb, "");
  StringCopy(server->configuration->database.mech_db, "mechs");
  StringCopy(server->configuration->database.map_db, "maps");
  server->configuration->btech_explode_reactor = 1;
  server->configuration->btech_explode_time = 120;
  server->configuration->btech_explode_ammo = 1;
  server->configuration->btech_explode_stop = 0;
  server->configuration->btech_stackpole = 1;
  server->configuration->btech_phys_use_pskill = 1;
  server->configuration->btech_erange = 1;
  server->configuration->btech_hit_arcs = 0;
  server->configuration->btech_dig_only_fs = 0;
  server->configuration->btech_digbonus = 3;
  server->configuration->btech_vcrit = 2;
  server->configuration->btech_dynspeed = 1;
  server->configuration->btech_ic = 1;
  server->configuration->btech_parts = 1;
  server->configuration->btech_slowdown = 2;
  server->configuration->btech_fasaturn = 1;
  server->configuration->btech_fasacrit = 0;
  server->configuration->btech_fasaadvvtolcrit = 0;
  server->configuration->btech_fasaadvvhlcrit = 0;
  server->configuration->btech_fasaadvvhlfire = 0;
  server->configuration->btech_divrotordamage = 0;
  server->configuration->btech_moddamagewithrange = 0;
  server->configuration->btech_moddamagewithwoods = 0;
  server->configuration->btech_hotloadaddshalfbthmod = 0;
  server->configuration->btech_nofusionvtolfuel = 0;
  server->configuration->btech_tankfriendly = 0;
  server->configuration->btech_newterrain = 0;
  server->configuration->btech_skidcliff = 0;
  server->configuration->btech_xp_bthmod = 0;
  server->configuration->btech_xp_missilemod = 100;
  server->configuration->btech_xp_ammomod = 100;
  server->configuration->btech_defaultweapdam = 5;
  server->configuration->btech_xp_modifier = 100;
  server->configuration->btech_defaultweapbv = 120;
  server->configuration->btech_xp_usePilotBVMod = 1;
  server->configuration->btech_oldxpsystem = 1;
  server->configuration->btech_xp_vrtmod = 0;
  server->configuration->btech_limitedrepairs = 0;
  server->configuration->btech_newcharge = 0;
  server->configuration->btech_tl3_charge = 0;
  server->configuration->btech_xploss = 666;
  server->configuration->btech_critlevel = 100;
  server->configuration->btech_tankshield = 0;
  server->configuration->btech_newstagger = 1;
  server->configuration->btech_newstaggertons = 1;
  server->configuration->btech_newstaggertime = 5;
  server->configuration->btech_extendedmovemod = 1;
  server->configuration->btech_stacking = 2;
  server->configuration->btech_stackdamage = 100;
  server->configuration->btech_mw_losmap = 1;
  server->configuration->btech_seismic_see_stopped = 0;
  server->configuration->btech_exile_stun_code = 0;
  server->configuration->btech_roll_on_backwalk = 1;
  server->configuration->btech_usedmechstore = 0;
  server->configuration->btech_ooc_comsys = 0;
  server->configuration->btech_idf_requires_spotter = 1;
  server->configuration->btech_vtol_ice_causes_fire = 1;
  server->configuration->btech_glancing_blows = 1;
  server->configuration->btech_inferno_penalty = 0;
  server->configuration->btech_perunit_xpmod = 1;
  server->configuration->btech_tsm_tow_bonus = 1;
  server->configuration->btech_tsm_sprint_bonus = 1;
  server->configuration->btech_heatcutoff = 1;
  server->configuration->btech_sprint_bth = -4;
  server->configuration->btech_cost_debug = 0;
  server->configuration->btech_noisy_xpgain = 0;
  server->configuration->btech_xpgain_cap = 10;
  server->configuration->btech_transported_unit_death = 1;
  server->configuration->btech_mwpickup_action = 1;
  server->configuration->btech_standcareful = 1;
  server->configuration->btech_maxtechtime = 600;
  server->configuration->btech_blzmapmode = 0;
  server->configuration->btech_extended_piloting = 1;
  server->configuration->btech_extended_gunnery = 1;
  server->configuration->btech_xploss_for_mw = 1;
  server->configuration->btech_variable_techtime = 0;
  server->configuration->btech_techtime_mod = 0;
  server->configuration->btech_statengine_obj = -1;
#ifdef BT_FREETECHTIME
  server->configuration->btech_freetechtime = 0;
#endif
#ifdef BT_COMPLEXREPAIRS
  server->configuration->btech_complexrepair = 1;
#endif
  server->configuration->allow_chanlurking = 0;
  server->configuration->afterlife_dbref = 220;
  server->configuration->port = 6250;
  server->configuration->conc_port = 6251;
  server->configuration->init_size = 1000;
  StringCopy(server->configuration->conn_file, "text/connect.txt");
  StringCopy(server->configuration->conn_dir, "");
  StringCopy(server->configuration->quit_file, "text/quit.txt");
  StringCopy(server->configuration->down_file, "text/down.txt");
  StringCopy(server->configuration->full_file, "text/full.txt");
  StringCopy(server->configuration->site_file, "text/badsite.txt");
  StringCopy(server->configuration->help_dir, "help");
  StringCopy(server->configuration->down_msg, "");
  StringCopy(server->configuration->full_msg, "");
  StringCopy(server->configuration->dump_msg, "");
  StringCopy(server->configuration->postdump_msg, "");
  StringCopy(server->configuration->fixed_home_msg, "");
  StringCopy(server->configuration->fixed_tel_msg, "");
  StringCopy(server->configuration->public_channel, "Public");
  server->configuration->indent_desc = 0;
  server->configuration->name_spaces = 1;
  server->configuration->fork_dump = 1;
  server->configuration->fork_vfork = 0;
  server->configuration->have_specials = 1;
  server->configuration->have_comsys = 1;
  server->configuration->have_macros = 1;
  server->configuration->have_zones = 1;
  server->configuration->paranoid_alloc = 0;
  server->configuration->max_players = -1;
  server->configuration->database.dump_interval = 3600;
  server->configuration->check_interval = 600;
  server->configuration->events_daily_hour = 7;
  server->configuration->dump_offset = 0;
  server->configuration->check_offset = 300;
  server->configuration->idle_timeout = 3600;
  server->configuration->conn_timeout = 120;
  server->configuration->idle_interval = 60;
  server->configuration->retry_limit = 3;
  server->configuration->player_password_length_limit = 64;
  server->configuration->password_hash_opslimit = 3;
  server->configuration->password_hash_memlimit = 12 * 1024 * 1024;
  server->configuration->login_attempt_burst = 3;
  server->configuration->login_attempt_refill = 10;
  server->configuration->login_hash_limit = 5;
  server->configuration->output_limit = 16384;
  server->configuration->use_http = 0;
  server->configuration->queuemax = 100;
  server->configuration->queue_chunk = 10;
  server->configuration->active_q_chunk = 10;
  server->configuration->ex_flags = 1;
  server->configuration->robot_speak = 1;
  server->configuration->pub_flags = 1;
  server->configuration->quiet_look = 1;
  server->configuration->exam_public = 1;
  server->configuration->read_rem_desc = 0;
  server->configuration->read_rem_name = 0;
  server->configuration->sweep_dark = 0;
  server->configuration->player_listen = 0;
  server->configuration->dark_sleepers = 1;
  server->configuration->see_own_dark = 1;
  server->configuration->idle_wiz_dark = 0;
  server->configuration->pemit_players = 0;
  server->configuration->pemit_any = 0;
  server->configuration->match_mine = 0;
  server->configuration->match_mine_pl = 0;
  server->configuration->switch_df_all = 1;
  server->configuration->fascist_tport = 0;
  server->configuration->trace_topdown = 1;
  server->configuration->trace_limit = 200;
  server->configuration->safe_unowned = 0;
  /*
   * -- ??? Running SC on a non-SC DB may cause problems
   */
  server->configuration->space_compress = 1;
  server->configuration->start_room = 0;
  server->configuration->start_home = -1;
  server->configuration->default_home = -1;
  server->configuration->master_room = -1;
  server->configuration->player_flags.word1 = 0;
  server->configuration->player_flags.word2 = 0;
  server->configuration->room_flags.word1 = 0;
  server->configuration->room_flags.word2 = 0;
  server->configuration->exit_flags.word1 = 0;
  server->configuration->exit_flags.word2 = 0;
  server->configuration->thing_flags.word1 = 0;
  server->configuration->thing_flags.word2 = 0;
  server->configuration->robot_flags.word1 = ROBOT;
  server->configuration->robot_flags.word2 = 0;
  server->configuration->vattr_flags = AF_ODARK;
  StringCopy(server->configuration->mud_name, "TinyMUX");
  server->configuration->timeslice = 100;
  server->configuration->cmd_quota_max = 100;
  server->configuration->cmd_quota_incr = 5;
  server->configuration->is_login_enabled = true;
  server->configuration->is_interpreter_enabled = true;
  server->configuration->is_checkpointing_enabled = true;
  server->configuration->is_db_check_enabled = true;
  server->configuration->is_idle_check_enabled = true;
  server->configuration->is_dequeue_enabled = true;
  server->configuration->is_event_check_enabled = true;
  server->configuration->log_options =
      LOG_ALWAYS | LOG_BUGS | LOG_SECURITY | LOG_NET | LOG_LOGIN | LOG_DBSAVES |
      LOG_CONFIGMODS | LOG_SHOUTS | LOG_STARTUP | LOG_WIZARD | LOG_PROBLEMS |
      LOG_PCREATES;
  server->configuration->log_info = LOGOPT_TIMESTAMP | LOGOPT_LOC;
  server->configuration->func_nest_lim = 50;
  server->configuration->func_invk_lim = 2500;
  server->configuration->ntfy_nest_lim = 20;
  server->configuration->lock_nest_lim = 20;
  server->configuration->parent_nest_lim = 10;
  server->configuration->zone_nest_lim = 20;
  server->configuration->stack_limit = 50;
  server->configuration->cache_trim = 0;
  server->configuration->cache_depth = CACHE_DEPTH;
  server->configuration->cache_width = CACHE_WIDTH;
  server->configuration->cache_names = 1;
  StringCopy(server->configuration->lua.directory, "lua");
  server->configuration->lua.instruction_limit = 100000;
  server->configuration->lua.memory_limit = 64 * 1024 * 1024;

  server->configuration->exit_parent = 0;
  server->configuration->room_parent = 0;
  server->configuration->player_parent = 0;
  server->configuration->player_zone = 0;
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_log_not_found: Log a 'parameter not found' error.
 */
void configuration_log_not_found(MuxServer *server, DbRef player,
                                 const char *cmd, const char *thingname,
                                 const char *thing) {
  char *buff;

  if (server->configuration->is_initializing) {
    log_error(&server->log, LOG_STARTUP, "CNF", "NFND", "%s: %s %s not found.",
              cmd, thingname, thing);
  } else {
    buff = alloc_lbuf("configuration_log_not_found");
    snprintf(buff, LBUF_SIZE, "%s %s not found", thingname, thing);
    notify(&server->background_command.evaluation, player, buff);
    free_lbuf(buff);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_log_syntax: Log a syntax error.
 */

void configuration_log_syntax(MuxServer *server, DbRef player, const char *cmd,
                              const char *template, const char *arg) {
  if (server->configuration->is_initializing) {
    log_error(&server->log, LOG_STARTUP, "CNF", "SYNTX", "%s: %s %s", cmd,
              template, arg);
  } else {
    notify_printf(&server->background_command.evaluation, player, "%s%s",
                  template, arg);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * cf_status_from_succfail: Return command status from succ and fail info
 */

static int cf_status_from_succfail(DbRef player, char *cmd, int success,
                                   int failure, MuxServer *server) {

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
    if (server->configuration->is_initializing) {
      log_error(&server->log, LOG_STARTUP, "CNF", "NDATA", "%s: Nothing to set",
                cmd);
    } else {
      notify(&server->background_command.evaluation, player, "Nothing to set");
    }
  }
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_int: Set integer parameter.
 */

static int cf_int(int *vp, char *str, long extra, DbRef player, char *cmd,
                  MuxServer *server) {
  /*
   * Copy the numeric value to the parameter
   */

  sscanf(str, "%d", vp);
  return 0;
}
/* *INDENT-OFF* */

/* ---------------------------------------------------------------------------
 * cf_bool: Set boolean parameter.
 */

NameTable bool_names[] = {
    {"true", 1, 0, 1}, {"false", 1, 0, 0}, {"yes", 1, 0, 1},  {"no", 1, 0, 0},
    {"1", 1, 0, 1},    {"0", 1, 0, 0},     {nullptr, 0, 0, 0}};

/* *INDENT-ON* */

static int cf_bool(int *vp, char *str, long extra, DbRef player, char *cmd,
                   MuxServer *server) {
  *vp = (int)name_table_search(&server->database, server->configuration, GOD,
                               bool_names, str);
  if (*vp < 0)
    *vp = (long)0;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_string: Set string parameter.
 */

static int cf_string(int *vp, char *str, long extra, DbRef player, char *cmd,
                     MuxServer *server) {
  int retval;

  /*
   * Copy the string to the buffer if it is not too big
   */

  retval = 0;
  if (strlen(str) >= (size_t)extra) {
    str[extra - 1] = '\0';
    if (server->configuration->is_initializing) {
      log_error(&server->log, LOG_STARTUP, "CNF", "NFND",
                "%s: String truncated", cmd);
    } else {
      notify(&server->background_command.evaluation, player,
             "String truncated");
    }
    retval = 1;
  }
  StringCopy((char *)vp, str);
  return retval;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_alias: define a generic hash table alias.
 */

static int cf_alias(void *vp, char *str, long extra, DbRef player, char *cmd,
                    MuxServer *server) {
  char *alias, *orig, *p;
  int *cp = nullptr;

  alias = strtok(str, " \t=,");
  orig = strtok(nullptr, " \t=,");
  if (orig) {
    for (p = orig; *p; p++)
      *p = ToLower(*p);
    cp = hash_table_find(orig, (HashTable *)vp);
    if (cp == nullptr) {
      for (p = orig; *p; p++)
        *p = ToUpper(*p);
      cp = hash_table_find(orig, (HashTable *)vp);
      if (cp == nullptr) {
        configuration_log_not_found(server, player, cmd, "Entry", orig);
        return -1;
      }
    }
  }

  if (cp == nullptr) {
    return -1;
  }

  hash_table_add(alias, cp, (HashTable *)vp);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_flagalias: define a flag alias.
 */

static int cf_flagalias(int *vp, char *str, long extra, DbRef player, char *cmd,
                        MuxServer *server) {
  char *alias, *orig;
  int *cp, success;

  success = 0;
  alias = strtok(str, " \t=,");
  orig = strtok(nullptr, " \t=,");

  cp = hash_table_find(orig, &server->world_indexes.flags);
  if (cp != nullptr) {
    hash_table_add(alias, cp, &server->world_indexes.flags);
    success++;
  }
  if (!success)
    configuration_log_not_found(server, player, cmd, "Flag", orig);
  return ((success > 0) ? 0 : -1);
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_modify_bits: set or clear bits in a flag word from a
 * namelist.
 */
int configuration_modify_bits(int *vp, char *str, long extra, DbRef player,
                              char *cmd, MuxServer *server) {
  char *sp;
  int f, negate, success, failure;

  /*
   * Walk through the tokens
   */

  success = failure = 0;
  sp = strtok(str, " \t");
  while (sp != nullptr) {

    /*
     * Check for negation
     */

    negate = 0;
    if (*sp == '!') {
      negate = 1;
      sp++;
    }
    /*
     * Set or clear the appropriate bit
     */

    f = name_table_search(&server->database, server->configuration, GOD,
                          (NameTable *)extra, sp);
    if (f > 0) {
      if (negate)
        *vp &= ~f;
      else
        *vp |= f;
      success++;
    } else {
      configuration_log_not_found(server, player, cmd, "Entry", sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok(nullptr, " \t");
  }
  return cf_status_from_succfail(player, cmd, success, failure, server);
}

/*
 * ---------------------------------------------------------------------------
 * * cf_set_flags: Clear flag word and then set from a flags htab.
 */

static int cf_set_flags(void *vp, char *str, long extra, DbRef player,
                        char *cmd, MuxServer *server) {
  char *sp;
  FLAGENT *fp;
  FLAGSET *fset;

  int success, failure;

  /*
   * Walk through the tokens
   */

  success = failure = 0;
  sp = strtok(str, " \t");
  fset = (FLAGSET *)vp;

  while (sp != nullptr) {

    /*
     * Set the appropriate bit
     */

    fp = (FLAGENT *)hash_table_find(sp, &server->world_indexes.flags);
    if (fp != nullptr) {
      if (success == 0) {
        (*fset).word1 = 0;
        (*fset).word2 = 0;
      }
      if (fp->flagflag & FLAG_WORD3)
        (*fset).word3 |= fp->flagvalue;
      else if (fp->flagflag & FLAG_WORD2)
        (*fset).word2 |= fp->flagvalue;
      else
        (*fset).word1 |= fp->flagvalue;
      success++;
    } else {
      configuration_log_not_found(server, player, cmd, "Entry", sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok(nullptr, " \t");
  }
  if ((success == 0) && (failure == 0)) {
    (*fset).word1 = 0;
    (*fset).word2 = 0;
    return 0;
  }
  if (success > 0)
    return ((failure == 0) ? 0 : 1);
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_badname: Disallow use of player name/alias.
 */

static int cf_badname(int *vp, char *str, long extra, DbRef player, char *cmd,
                      MuxServer *server) {
  if (extra)
    badname_remove(&server->world, str);
  else
    badname_add(&server->world, str);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_site: Update site information
 */

static int cf_site(long **vp, char *str, long extra, DbRef player, char *cmd,
                   MuxServer *server) {
  SiteData *site, *last, *head;
  char *addr_txt, *mask_txt;
  struct in_addr addr_num, mask_num;

  addr_txt = strtok(str, " \t=,");
  mask_txt = nullptr;
  if (addr_txt)
    mask_txt = strtok(nullptr, " \t=,");
  if (!addr_txt || !*addr_txt || !mask_txt || !*mask_txt) {
    configuration_log_syntax(server, player, cmd,
                             "Missing host address or mask.", "");
    return -1;
  }

  addr_num.s_addr = inet_addr(addr_txt);
  mask_num.s_addr = inet_addr(mask_txt);

  if (addr_num.s_addr == INADDR_NONE) {
    configuration_log_syntax(server, player, cmd,
                             "Bad host address: ", addr_txt);
    return -1;
  }
  head = (SiteData *)*vp;
  /*
   * Parse the access entry and allocate space for it
   */

  site = malloc(sizeof(SiteData));

  /*
   * Initialize the site entry
   */

  site->address.s_addr = addr_num.s_addr;
  site->mask.s_addr = mask_num.s_addr;
  site->flag = (int)extra;
  site->next = nullptr;

  /*
   * Link in the entry.  Link it at the start if not initializing, at *
   *
   * *  * *  * *  * *  * * the end if initializing.  This is so that
   * entries  * in * the * config * * * file are processed as you would
   * think they * * would be, * while * entries * * made while running
   * are processed * * first.
   */

  if (server->configuration->is_initializing) {
    if (head == nullptr) {
      *vp = (long *)site;
    } else {
      for (last = head; last->next; last = last->next)
        ;
      last->next = site;
    }
  } else {
    site->next = head;
    *vp = (long *)site;
  }
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_cf_access: Set access on config directives
 */

static int cf_cf_access(int *vp, char *str, long extra, DbRef player, char *cmd,
                        MuxServer *server) {
  CONF *tp;
  char *ap;

  for (ap = str; *ap && !isspace(*ap); ap++)
    ;
  if (*ap)
    *ap++ = '\0';

  for (tp = conftable; tp->pname; tp++) {
    if (!strcmp(tp->pname, str)) {
      return configuration_modify_bits(&tp->flags, ap, extra, player, cmd,
                                       server);
    }
  }
  configuration_log_not_found(server, player, cmd, "Config directive", str);
  return -1;
}

/* ---------------------------------------------------------------------------
 * conftable: Table for parsing the configuration file.
 */

DEFINE_CONFIGURATION_ADAPTER(cf_access)
DEFINE_CONFIGURATION_ADAPTER(cf_acmd_access)
DEFINE_CONFIGURATION_ADAPTER(cf_alias)
DEFINE_CONFIGURATION_ADAPTER(cf_attr_access)
DEFINE_CONFIGURATION_ADAPTER(cf_badname)
DEFINE_CONFIGURATION_ADAPTER(cf_bool)
DEFINE_CONFIGURATION_ADAPTER(cf_cf_access)
DEFINE_CONFIGURATION_ADAPTER(cf_cmd_alias)
DEFINE_CONFIGURATION_ADAPTER(cf_flagalias)
DEFINE_CONFIGURATION_ADAPTER(cf_func_access)
DEFINE_CONFIGURATION_ADAPTER(cf_int)
DEFINE_CONFIGURATION_ADAPTER(cf_ntab_access)
DEFINE_CONFIGURATION_ADAPTER(cf_set_flags)
DEFINE_CONFIGURATION_ADAPTER(cf_site)
DEFINE_CONFIGURATION_ADAPTER(cf_string)
DEFINE_CONFIGURATION_ADAPTER(configuration_modify_bits)

CONF conftable[] = {
    {"access", cf_access_configuration_adapter, CA_GOD, nullptr,
     (long)access_nametab},
    {"alias", cf_cmd_alias_configuration_adapter, CA_GOD, COMMAND_LOC(commands),
     0},
    {"attr_access", cf_attr_access_configuration_adapter, CA_GOD, nullptr,
     (long)attraccess_nametab},
    {"attr_alias", cf_alias_configuration_adapter, CA_GOD,
     WORLD_LOC(attributes), 0},
    {"attr_cmd_access", cf_acmd_access_configuration_adapter, CA_GOD, nullptr,
     (long)access_nametab},
    {"bad_name", cf_badname_configuration_adapter, CA_GOD, nullptr, 0},
    {"badsite_file", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(site_file), 32},
    {"btech_explode_reactor", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_explode_reactor), 0},
    {"btech_explode_time", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_explode_time), 0},
    {"btech_explode_ammo", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_explode_ammo), 0},
    {"btech_explode_stop", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_explode_stop), 0},
    {"btech_parts", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_parts), 0},
    {"btech_ic", cf_int_configuration_adapter, CA_GOD, CONFIG_LOC(btech_ic), 0},
    {"btech_afterlife_dbref", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(afterlife_dbref), 0},
    {"btech_vcrit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_vcrit), 0},
    {"btech_dynspeed", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_dynspeed), 0},
    {"btech_slowdown", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_slowdown), 0},
    {"btech_fasaturn", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_fasaturn), 0},
    {"btech_fasacrit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_fasacrit), 0},
    {"btech_fasaadvvtolcrit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_fasaadvvtolcrit), 0},
    {"btech_fasaadvvhlcrit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_fasaadvvhlcrit), 0},
    {"btech_fasaadvvhlfire", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_fasaadvvhlfire), 0},
    {"btech_divrotordamage", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_divrotordamage), 0},
    {"btech_moddamagewithrange", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_moddamagewithrange), 0},
    {"btech_moddamagewithwoods", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_moddamagewithwoods), 0},
    {"btech_hotloadaddshalfbthmod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_hotloadaddshalfbthmod), 0},
    {"btech_nofusionvtolfuel", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_nofusionvtolfuel), 0},
    {"btech_tankfriendly", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_tankfriendly), 0},
    {"btech_newcharge", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_newcharge), 0},
    {"btech_tl3_charge", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_tl3_charge), 0},
    {"btech_newterrain", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_newterrain), 0},
    {"btech_xploss", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xploss), 0},
    {"btech_critlevel", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_critlevel), 0},
    {"btech_tankshield", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_tankshield), 0},
    {"btech_newstagger", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_newstagger), 0},
    {"btech_newstaggertons", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_newstaggertons), 0},
    {"btech_newstaggertime", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_newstaggertime), 0},
    {"allow_chanlurking", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(allow_chanlurking), 0},
    {"btech_skidcliff", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_skidcliff), 0},
    {"btech_xp_bthmod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xp_bthmod), 0},
    {"btech_xp_missilemod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xp_missilemod), 0},
    {"btech_xp_ammomod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xp_ammomod), 0},
    {"btech_defaultweapdam", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_defaultweapdam), 0},
    {"btech_xp_modifier", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xp_modifier), 0},
    {"btech_defaultweapbv", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_defaultweapbv), 0},
    {"btech_xp_usePilotBVMod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xp_usePilotBVMod), 0},
    {"btech_oldxpsystem", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_oldxpsystem), 0},
    {"btech_xp_vrtmod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xp_vrtmod), 0},
    {"btech_extendedmovemod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_extendedmovemod), 0},
    {"btech_stacking", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_stacking), 0},
    {"btech_stackdamage", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_stackdamage), 0},
    {"btech_mw_losmap", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_mw_losmap), 0},
    {"btech_exile_stun_code", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_exile_stun_code), 0},
    {"btech_roll_on_backwalk", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_roll_on_backwalk), 0},
    {"btech_usedmechstore", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_usedmechstore), 0},
    {"btech_ooc_comsys", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_ooc_comsys), 0},
    {"btech_idf_requires_spotter", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_idf_requires_spotter), 0},
    {"btech_perunit_xpmod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_perunit_xpmod), 0},
    {"btech_tsm_tow_bonus", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_tsm_tow_bonus), 0},
    {"btech_heatcutoff", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_heatcutoff), 0},
    {"btech_cost_debug", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_cost_debug), 0},
    {"btech_noisy_xpgain", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_noisy_xpgain), 0},
    {"btech_xpgain_cap", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xpgain_cap), 0},
    {"btech_transported_unit_death", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_transported_unit_death), 0},
    {"btech_mwpickup_action", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_mwpickup_action), 0},
    {"btech_standcareful", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_standcareful), 0},
    {"btech_maxtechtime", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_maxtechtime), 0},
    {"btech_sprint_bth", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_sprint_bth), 0},
    {"btech_tsm_sprint_bonus", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_tsm_sprint_bonus), 0},
    {"btech_vtol_ice_causes_fire", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_vtol_ice_causes_fire), 0},
    {"btech_glancing_blows", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_glancing_blows), 0},
    {"btech_inferno_penalty", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_inferno_penalty), 0},
    {"btech_blzmapmode", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_blzmapmode), 0},
    {"btech_extended_piloting", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_extended_piloting), 0},
    {"btech_extended_gunnery", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_extended_gunnery), 0},
    {"btech_xploss_for_mw", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_xploss_for_mw), 0},
    {"btech_variable_techtime", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_variable_techtime), 0},
    {"btech_techtime_mod", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_techtime_mod), 0},
    {"btech_statengine_obj", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_statengine_obj), 0},
#ifdef BT_FREETECHTIME
    {"btech_freetechtime", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_freetechtime), 0},
#endif
#ifdef BT_COMPLEXREPAIRS
    {"btech_complexrepair", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_complexrepair), 0},
#endif
    {"btech_seismic_see_stopped", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_seismic_see_stopped), 0},
    {"btech_limitedrepairs", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_limitedrepairs), 0},
    {"btech_stackpole", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_stackpole), 0},
    {"btech_phys_use_pskill", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_phys_use_pskill), 0},
    {"btech_erange", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_erange), 0},
    {"btech_hit_arcs", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_hit_arcs), 0},
    {"btech_dig_only_fs", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_dig_only_fs), 0},
    {"btech_digbonus", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(btech_digbonus), 0},
    {"cache_depth", cf_int_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(cache_depth), 0},
    {"cache_names", cf_bool_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(cache_names), 0},
    {"cache_trim", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(cache_trim), 0},
    {"cache_width", cf_int_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(cache_width), 0},
    {"check_interval", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(check_interval), 0},
    {"check_offset", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(check_offset), 0},
    {"command_quota_increment", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(cmd_quota_incr), 0},
    {"command_quota_max", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(cmd_quota_max), 0},
    {"concentrator_port", cf_int_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(conc_port), 0},
    {"config_access", cf_cf_access_configuration_adapter, CA_GOD, nullptr,
     (long)access_nametab},
    {"conn_timeout", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(conn_timeout), 0},
    {"connect_dir", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(conn_dir), 32},
    {"connect_file", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(conn_file), 32},
    {"dark_sleepers", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(dark_sleepers), 0},
    {"default_home", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(default_home), 0},
    {"down_file", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(down_file), 32},
    {"down_message", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(down_msg), 4096},
    {"dump_interval", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(database.dump_interval), 0},
    {"dump_message", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(dump_msg), 128},
    {"postdump_message", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(postdump_msg), 128},
    {"dump_offset", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(dump_offset), 0},
    {"examine_flags", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(ex_flags), 0},
    {"examine_public_attrs", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(exam_public), 0},
    {"exit_flags", cf_set_flags_configuration_adapter, CA_GOD,
     (int *)CONFIG_LOC(exit_flags), 0},
    {"events_daily_hour", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(events_daily_hour), 0},
    {"fascist_teleport", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(fascist_tport), 0},
    {"fixed_home_message", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(fixed_home_msg), 128},
    {"fixed_tel_message", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(fixed_tel_msg), 128},
    {"flag_alias", cf_flagalias_configuration_adapter, CA_GOD, nullptr, 0},
    {"forbid_site", cf_site_configuration_adapter, CA_GOD,
     ACCESS_LOC(access_sites), H_FORBIDDEN},
    {"fork_dump", cf_bool_configuration_adapter, CA_GOD, CONFIG_LOC(fork_dump),
     0},
    {"fork_vfork", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(fork_vfork), 0},
    {"full_file", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(full_file), 32},
    {"full_message", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(full_msg), 4096},
    {"function_access", cf_func_access_configuration_adapter, CA_GOD, nullptr,
     (long)access_nametab},
    {"function_alias", cf_alias_configuration_adapter, CA_GOD,
     COMMAND_LOC(functions), 0},
    {"function_invocation_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(func_invk_lim), 0},
    {"function_recursion_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(func_nest_lim), 0},
    {"game_database", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(database.gamedb),
     sizeof(((ServerConfiguration *)nullptr)->database.gamedb)},
    {"good_name", cf_badname_configuration_adapter, CA_GOD, nullptr, 1},
    {"have_specials", cf_bool_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(have_specials), 0},
    {"have_comsys", cf_bool_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(have_comsys), 0},
    {"have_macros", cf_bool_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(have_macros), 0},
    {"have_zones", cf_bool_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(have_zones), 0},
    {"help_directory", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(help_dir), sizeof(((ServerConfiguration *)nullptr)->help_dir)},
    {"use_http", cf_bool_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(use_http), 0},
    {"idle_wiz_dark", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(idle_wiz_dark), 0},
    {"idle_interval", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(idle_interval), 0},
    {"idle_timeout", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(idle_timeout), 0},
    {"indent_desc", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(indent_desc), 0},
    {"initial_size", cf_int_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(init_size), 0},
    {"list_access", cf_ntab_access_configuration_adapter, CA_GOD,
     (int *)list_names, (long)access_nametab},
    {"lock_recursion_limit", cf_int_configuration_adapter, CA_WIZARD,
     CONFIG_LOC(lock_nest_lim), 0},
    {"lua_directory", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(lua.directory),
     sizeof(((ServerConfiguration *)nullptr)->lua.directory)},
    {"lua_instruction_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(lua.instruction_limit), 0},
    {"lua_memory_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(lua.memory_limit), 0},
    {"log", configuration_modify_bits_configuration_adapter, CA_GOD,
     CONFIG_LOC(log_options), (long)logoptions_nametab},
    {"log_options", configuration_modify_bits_configuration_adapter, CA_GOD,
     CONFIG_LOC(log_info), (long)logdata_nametab},
    {"map_database", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(database.map_db),
     sizeof(((ServerConfiguration *)nullptr)->database.map_db)},
    {"master_room", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(master_room), 0},
    {"match_own_commands", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(match_mine), 0},
    {"max_players", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(max_players), 0},
    {"mech_database", cf_string_configuration_adapter, CA_GOD,
     CONFIG_LOC(database.mech_db),
     sizeof(((ServerConfiguration *)nullptr)->database.mech_db)},
    {"mud_name", cf_string_configuration_adapter, CA_GOD, CONFIG_LOC(mud_name),
     32},
    {"notify_recursion_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(ntfy_nest_lim), 0},
    {"output_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(output_limit), 0},
    {"paranoid_allocate", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(paranoid_alloc), 0},
    {"parent_recursion_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(parent_nest_lim), 0},
    {"pemit_far_players", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(pemit_players), 0},
    {"password_hash_memlimit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(password_hash_memlimit), 0},
    {"password_hash_opslimit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(password_hash_opslimit), 0},
    {"pemit_any_object", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(pemit_any), 0},
    {"permit_site", cf_site_configuration_adapter, CA_GOD,
     ACCESS_LOC(access_sites), 0},
    {"player_flags", cf_set_flags_configuration_adapter, CA_GOD,
     (int *)CONFIG_LOC(player_flags), 0},
    {"player_listen", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(player_listen), 0},
    {"player_match_own_commands", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(match_mine_pl), 0},
    {"player_password_length_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(player_password_length_limit), 0},
    {"player_name_spaces", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(name_spaces), 0},
    {"player_queue_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(queuemax), 0},
    {"player_starting_home", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(start_home), 0},
    {"player_starting_room", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(start_room), 0},
    {"public_channel", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(public_channel), 32},
    {"port", cf_int_configuration_adapter, CA_DISABLED, CONFIG_LOC(port), 0},
    {"public_flags", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(pub_flags), 0},
    {"queue_active_chunk", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(active_q_chunk), 0},
    {"queue_idle_chunk", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(queue_chunk), 0},
    {"quiet_look", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(quiet_look), 0},
    {"quit_file", cf_string_configuration_adapter, CA_DISABLED,
     CONFIG_LOC(quit_file), 32},
    {"read_remote_desc", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(read_rem_desc), 0},
    {"read_remote_name", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(read_rem_name), 0},
    {"retry_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(retry_limit), 0},
    {"login_attempt_burst", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(login_attempt_burst), 0},
    {"login_attempt_refill", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(login_attempt_refill), 0},
    {"login_hash_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(login_hash_limit), 0},
    {"robot_flags", cf_set_flags_configuration_adapter, CA_GOD,
     (int *)CONFIG_LOC(robot_flags), 0},
    {"robot_speech", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(robot_speak), 0},
    {"room_flags", cf_set_flags_configuration_adapter, CA_GOD,
     (int *)CONFIG_LOC(room_flags), 0},
    {"see_owned_dark", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(see_own_dark), 0},
    {"show_unfindable_who", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(show_unfindable_who), 1},
    {"space_compress", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(space_compress), 0},
    {"stack_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(stack_limit), 0},
    {"suspect_site", cf_site_configuration_adapter, CA_GOD,
     ACCESS_LOC(suspect_sites), H_SUSPECT},
    {"sweep_dark", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(sweep_dark), 0},
    {"switch_default_all", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(switch_df_all), 0},
    {"thing_flags", cf_set_flags_configuration_adapter, CA_GOD,
     (int *)CONFIG_LOC(thing_flags), 0},
    {"timeslice", cf_int_configuration_adapter, CA_GOD, CONFIG_LOC(timeslice),
     0},
    {"trace_output_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(trace_limit), 0},
    {"trace_topdown", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(trace_topdown), 0},
    {"trust_site", cf_site_configuration_adapter, CA_GOD,
     ACCESS_LOC(suspect_sites), 0},
    {"unowned_safe", cf_bool_configuration_adapter, CA_GOD,
     CONFIG_LOC(safe_unowned), 0},
    {"user_attr_access", configuration_modify_bits_configuration_adapter,
     CA_GOD, CONFIG_LOC(vattr_flags), (long)attraccess_nametab},
    {"zone_recursion_limit", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(zone_nest_lim), 0},
    {"exit_parent", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(exit_parent), 0},
    {"room_parent", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(room_parent), 0},
    {"player_parent", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(player_parent), 0},
    {"player_zone", cf_int_configuration_adapter, CA_GOD,
     CONFIG_LOC(player_zone), 0},
    {nullptr, nullptr, 0, nullptr, 0}};

/*
 * ---------------------------------------------------------------------------
 * * configuration_set: Set config parameter.
 */
int configuration_set(MuxServer *server, char *cp, char *ap, DbRef player) {
  CONF *tp;
  int i;
  char *buff = nullptr;

  /*
   * Search the config parameter table for the command. If we find it,
   * call the handler to parse the argument.
   */

  for (tp = conftable; tp->pname; tp++) {
    if (!strcmp(tp->pname, cp)) {
      if (!server->configuration->is_initializing &&
          !check_access(&server->database, server->configuration, player,
                        tp->flags)) {
        notify(&server->background_command.evaluation, player,
               "Permission denied.");
        return (-1);
      }
      buff = alloc_lbuf("configuration_set");
      StringCopy(buff, ap);
      i = tp->interpreter(configuration_resolve_location(server, tp), ap,
                          tp->extra, player, cp, server);
      if (!server->configuration->is_initializing) {
        log_error(&server->log, LOG_CONFIGMODS, "CFG", "UPDAT",
                  "%s entered config directive: %s with args '%s'. Status: %s",
                  game_object_name(&server->database, player), cp, buff,
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

  configuration_log_not_found(server, player, "Set", "Config directive", cp);
  return (-1);
}

/*
 * ---------------------------------------------------------------------------
 * * do_admin: Command handler to set config params at runtime
 */
void do_admin(CommandInvocation *invocation) {
  int i;

  i = configuration_set(invocation->context->server, invocation->first,
                        invocation->second, invocation->player);
  if ((i >= 0) &&
      !is_quiet(&invocation->context->server->database, invocation->player))
    notify(&invocation->context->evaluation, invocation->player, "Set.");
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_toml_dispatch_to_set: Adapter from the TOML loader's
 * dispatch callback into configuration_set().
 */
static int configuration_toml_dispatch_to_set(const char *pname,
                                              const char *args, void *ctx) {
  MuxServer *server = ctx;
  /* configuration_set()'s (char *, char *) signature isn't const-correct;
     it only reads these strings. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  return configuration_set(server, (char *)pname, (char *)args, 0);
#pragma clang diagnostic pop
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_read: Read in config parameters from named file
 */
int configuration_read(MuxServer *server, char *fn) {
  char errbuf[256];
  bool ok;

  StringCopy(server->configuration->config_file, fn);
  server->configuration->is_initializing = true;
  ok = configuration_toml_load(fn, configuration_toml_dispatch_to_set, server,
                               errbuf, sizeof(errbuf));
  server->configuration->is_initializing = false;
  if (!ok) {
    fprintf(stderr, "Error reading config file '%s': %s\n", fn, errbuf);
    return -1;
  }
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_list_access: List access to config directives.
 */
void configuration_list_access(MuxServer *server, DbRef player) {
  CONF *tp;
  char *buff;

  buff = alloc_mbuf("configuration_list_access");
  for (tp = conftable; tp->pname; tp++) {
    if (is_god(&server->database, player) ||
        check_access(&server->database, server->configuration, player,
                     tp->flags)) {
      snprintf(buff, MBUF_SIZE, "%s:", tp->pname);
      name_table_list_set(&server->background_command.evaluation,
                          server->configuration, player, access_nametab,
                          tp->flags, buff, 1);
    }
  }
  free_mbuf(buff);
}

/* ----------------------------------------------------------------------
 ** fun_config: returns a server configuration option
 */
void fun_config(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs,
                EvaluationContext *context) {
  CONF *cp;

  for (cp = conftable; cp->pname; ++cp) {
    if (!strcmp(cp->pname, fargs[0])) {
      /* ::FIX:: [cad] little hack. I don't think it's necessairy to need god
       *privs
       ** to read options so check_access doesn't work
       */
      if (cp->flags == CA_DISABLED) {
        safe_str("#-1 PERMISSION DENIED", buff, bufc);
        return;
      }
      if (cp->interpreter == cf_string_configuration_adapter) {
        safe_str(configuration_resolve_location(context->server, cp), buff,
                 bufc);
        return;
      }

      /* [cad] bool can be returned as 0|1 or true|false softcoders should
         decide how they want it */
      if (cp->interpreter == cf_int_configuration_adapter ||
          cp->interpreter == cf_bool_configuration_adapter) {
        safe_tprintf_str(
            buff, bufc, "%d",
            *(int *)configuration_resolve_location(context->server, cp));
        return;
      }

      /* [cad] no other idea what to do with the hashtables and stuff */
      safe_str("#-1 UNCONVERTABLE CONF TYPE", buff, bufc);
      return;
    }
  }
  safe_str("#-1", buff, bufc);
}
