/* configuration.c - Configuration parsing and defaults */

#include "mux/server/configuration.h"

#include "mux/server/platform.h"

#include <arpa/inet.h>

#include "mux/commands/command.h"
#include "mux/commands/functions.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
/* default (runtime-resettable) cache parameters */

#define CACHE_DEPTH 10
#define CACHE_WIDTH 20

/*
 * ---------------------------------------------------------------------------
 * * CONFPARM: Data used to find fields in ServerConfiguration.
 */

typedef struct confparm CONF;
struct confparm {
  char *pname;       /* parm name */
  void *interpreter; /* routine to interp parameter */
  int flags;         /* control flags */
  int *loc;          /* where to store value */
  long extra;        /* extra data for interpreter */
};

/*
 * ---------------------------------------------------------------------------
 * * External symbols.
 */

ServerConfiguration mudconf;

extern NameTable logdata_nametab[];
extern NameTable logoptions_nametab[];
extern NameTable access_nametab[];
extern NameTable attraccess_nametab[];
extern NameTable list_names[];
extern CONF conftable[];

/*
 * ---------------------------------------------------------------------------
 * * configuration_initialize: Initialize mudconf to default values.
 */

void configuration_initialize(void) {
  StringCopy(mudconf.gamedb, "");
  StringCopy(mudconf.mech_db, "mechs");
  StringCopy(mudconf.map_db, "maps");
  mudconf.allow_unloggedwho = 0;
  mudconf.btech_explode_reactor = 1;
  mudconf.btech_explode_time = 120;
  mudconf.btech_explode_ammo = 1;
  mudconf.btech_explode_stop = 0;
  mudconf.btech_stackpole = 1;
  mudconf.btech_phys_use_pskill = 1;
  mudconf.btech_erange = 1;
  mudconf.btech_hit_arcs = 0;
  mudconf.btech_dig_only_fs = 0;
  mudconf.btech_digbonus = 3;
  mudconf.btech_vcrit = 2;
  mudconf.btech_dynspeed = 1;
  mudconf.btech_ic = 1;
  mudconf.btech_parts = 1;
  mudconf.btech_slowdown = 2;
  mudconf.btech_fasaturn = 1;
  mudconf.btech_fasacrit = 0;
  mudconf.btech_fasaadvvtolcrit = 0;
  mudconf.btech_fasaadvvhlcrit = 0;
  mudconf.btech_fasaadvvhlfire = 0;
  mudconf.btech_divrotordamage = 0;
  mudconf.btech_moddamagewithrange = 0;
  mudconf.btech_moddamagewithwoods = 0;
  mudconf.btech_hotloadaddshalfbthmod = 0;
  mudconf.btech_nofusionvtolfuel = 0;
  mudconf.btech_tankfriendly = 0;
  mudconf.btech_newterrain = 0;
  mudconf.btech_skidcliff = 0;
  mudconf.btech_xp_bthmod = 0;
  mudconf.btech_xp_missilemod = 100;
  mudconf.btech_xp_ammomod = 100;
  mudconf.btech_defaultweapdam = 5;
  mudconf.btech_xp_modifier = 100;
  mudconf.btech_defaultweapbv = 120;
  mudconf.btech_xp_usePilotBVMod = 1;
  mudconf.btech_oldxpsystem = 1;
  mudconf.btech_xp_vrtmod = 0;
  mudconf.btech_limitedrepairs = 0;
  mudconf.btech_newcharge = 0;
  mudconf.btech_tl3_charge = 0;
  mudconf.btech_xploss = 666;
  mudconf.btech_critlevel = 100;
  mudconf.btech_tankshield = 0;
  mudconf.btech_newstagger = 1;
  mudconf.btech_newstaggertons = 1;
  mudconf.btech_newstaggertime = 5;
  mudconf.btech_extendedmovemod = 1;
  mudconf.btech_stacking = 2;
  mudconf.btech_stackdamage = 100;
  mudconf.btech_mw_losmap = 1;
  mudconf.btech_seismic_see_stopped = 0;
  mudconf.btech_exile_stun_code = 0;
  mudconf.btech_roll_on_backwalk = 1;
  mudconf.btech_usedmechstore = 0;
  mudconf.btech_ooc_comsys = 0;
  mudconf.btech_idf_requires_spotter = 1;
  mudconf.btech_vtol_ice_causes_fire = 1;
  mudconf.btech_glancing_blows = 1;
  mudconf.btech_inferno_penalty = 0;
  mudconf.btech_perunit_xpmod = 1;
  mudconf.btech_tsm_tow_bonus = 1;
  mudconf.btech_tsm_sprint_bonus = 1;
  mudconf.btech_heatcutoff = 1;
  mudconf.btech_sprint_bth = -4;
  mudconf.btech_cost_debug = 0;
  mudconf.btech_noisy_xpgain = 0;
  mudconf.btech_xpgain_cap = 10;
  mudconf.btech_transported_unit_death = 1;
  mudconf.btech_mwpickup_action = 1;
  mudconf.btech_standcareful = 1;
  mudconf.btech_maxtechtime = 600;
  mudconf.btech_blzmapmode = 0;
  mudconf.btech_extended_piloting = 1;
  mudconf.btech_extended_gunnery = 1;
  mudconf.btech_xploss_for_mw = 1;
  mudconf.btech_variable_techtime = 0;
  mudconf.btech_techtime_mod = 0;
  mudconf.btech_statengine_obj = -1;
#ifdef BT_FREETECHTIME
  mudconf.btech_freetechtime = 0;
#endif
#ifdef BT_COMPLEXREPAIRS
  mudconf.btech_complexrepair = 1;
#endif
  mudconf.namechange_days = 60;
  mudconf.allow_chanlurking = 0;
  mudconf.afterlife_dbref = 220;
  mudconf.port = 6250;
  mudconf.conc_port = 6251;
  mudconf.init_size = 1000;
  StringCopy(mudconf.conn_file, "text/connect.txt");
  StringCopy(mudconf.conn_dir, "");
  StringCopy(mudconf.creg_file, "text/register.txt");
  StringCopy(mudconf.regf_file, "text/create_reg.txt");
  StringCopy(mudconf.quit_file, "text/quit.txt");
  StringCopy(mudconf.down_file, "text/down.txt");
  StringCopy(mudconf.full_file, "text/full.txt");
  StringCopy(mudconf.site_file, "text/badsite.txt");
  StringCopy(mudconf.crea_file, "text/newuser.txt");
  StringCopy(mudconf.help_file, "text/help.txt");
  StringCopy(mudconf.help_indx, "text/help.indx");
  StringCopy(mudconf.whelp_file, "text/wizhelp.txt");
  StringCopy(mudconf.whelp_indx, "text/wizhelp.indx");
  StringCopy(mudconf.down_msg, "");
  StringCopy(mudconf.full_msg, "");
  StringCopy(mudconf.dump_msg, "");
  StringCopy(mudconf.postdump_msg, "");
  StringCopy(mudconf.fixed_home_msg, "");
  StringCopy(mudconf.fixed_tel_msg, "");
  StringCopy(mudconf.public_channel, "Public");
  mudconf.indent_desc = 0;
  mudconf.name_spaces = 1;
  mudconf.fork_dump = 1;
  mudconf.fork_vfork = 0;
  mudconf.have_specials = 1;
  mudconf.have_comsys = 1;
  mudconf.have_macros = 1;
  mudconf.have_zones = 1;
  mudconf.paranoid_alloc = 0;
  mudconf.max_players = -1;
  mudconf.dump_interval = 3600;
  mudconf.check_interval = 600;
  mudconf.events_daily_hour = 7;
  mudconf.dump_offset = 0;
  mudconf.check_offset = 300;
  mudconf.idle_timeout = 3600;
  mudconf.conn_timeout = 120;
  mudconf.idle_interval = 60;
  mudconf.retry_limit = 3;
  mudconf.output_limit = 16384;
  mudconf.use_http = 0;
  mudconf.queuemax = 100;
  mudconf.queue_chunk = 10;
  mudconf.active_q_chunk = 10;
  mudconf.ex_flags = 1;
  mudconf.robot_speak = 1;
  mudconf.pub_flags = 1;
  mudconf.quiet_look = 1;
  mudconf.exam_public = 1;
  mudconf.read_rem_desc = 0;
  mudconf.read_rem_name = 0;
  mudconf.sweep_dark = 0;
  mudconf.player_listen = 0;
  mudconf.dark_sleepers = 1;
  mudconf.see_own_dark = 1;
  mudconf.idle_wiz_dark = 0;
  mudconf.pemit_players = 0;
  mudconf.pemit_any = 0;
  mudconf.match_mine = 0;
  mudconf.match_mine_pl = 0;
  mudconf.switch_df_all = 1;
  mudconf.fascist_tport = 0;
  mudconf.trace_topdown = 1;
  mudconf.trace_limit = 200;
  mudconf.safe_unowned = 0;
  /*
   * -- ??? Running SC on a non-SC DB may cause problems
   */
  mudconf.space_compress = 1;
  mudconf.start_room = 0;
  mudconf.start_home = -1;
  mudconf.default_home = -1;
  mudconf.master_room = -1;
  mudconf.player_flags.word1 = 0;
  mudconf.player_flags.word2 = 0;
  mudconf.room_flags.word1 = 0;
  mudconf.room_flags.word2 = 0;
  mudconf.exit_flags.word1 = 0;
  mudconf.exit_flags.word2 = 0;
  mudconf.thing_flags.word1 = 0;
  mudconf.thing_flags.word2 = 0;
  mudconf.robot_flags.word1 = ROBOT;
  mudconf.robot_flags.word2 = 0;
  mudconf.vattr_flags = AF_ODARK;
  StringCopy(mudconf.mud_name, "TinyMUX");
  mudconf.timeslice = 100;
  mudconf.cmd_quota_max = 100;
  mudconf.cmd_quota_incr = 5;
  mudconf.control_flags = 0xffffffff; /*
                                       * Everything for now...
                                       */
  mudconf.log_options = LOG_ALWAYS | LOG_BUGS | LOG_SECURITY | LOG_NET |
                        LOG_LOGIN | LOG_DBSAVES | LOG_CONFIGMODS | LOG_SHOUTS |
                        LOG_STARTUP | LOG_WIZARD | LOG_PROBLEMS | LOG_PCREATES;
  mudconf.log_info = LOGOPT_TIMESTAMP | LOGOPT_LOC;
  mudconf.markdata[0] = 0x01;
  mudconf.markdata[1] = 0x02;
  mudconf.markdata[2] = 0x04;
  mudconf.markdata[3] = 0x08;
  mudconf.markdata[4] = 0x10;
  mudconf.markdata[5] = 0x20;
  mudconf.markdata[6] = 0x40;
  mudconf.markdata[7] = 0x80;
  mudconf.func_nest_lim = 50;
  mudconf.func_invk_lim = 2500;
  mudconf.ntfy_nest_lim = 20;
  mudconf.lock_nest_lim = 20;
  mudconf.parent_nest_lim = 10;
  mudconf.zone_nest_lim = 20;
  mudconf.stack_limit = 50;
  mudconf.cache_trim = 0;
  mudconf.cache_depth = CACHE_DEPTH;
  mudconf.cache_width = CACHE_WIDTH;
  mudconf.cache_names = 1;
  StringCopy(mudconf.lua_directory, "lua");
  mudconf.lua_instruction_limit = 100000;
  mudconf.lua_memory_limit = 64 * 1024 * 1024;

  mudconf.exit_parent = 0;
  mudconf.room_parent = 0;
  mudconf.player_parent = 0;
  mudconf.player_zone = 0;
  server_state_initialize();
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_log_not_found: Log a 'parameter not found' error.
 */
void configuration_log_not_found(DbRef player, char *cmd, const char *thingname,
                                 char *thing) {
  char *buff;

  if (mudstate.initializing) {
    log_error(LOG_STARTUP, "CNF", "NFND", "%s: %s %s not found.", cmd,
              thingname, thing);
  } else {
    buff = alloc_lbuf("configuration_log_not_found");
    snprintf(buff, LBUF_SIZE, "%s %s not found", thingname, thing);
    notify(player, buff);
    free_lbuf(buff);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_log_syntax: Log a syntax error.
 */

void configuration_log_syntax(DbRef player, char *cmd, const char *template,
                              char *arg) {
  if (mudstate.initializing) {
    log_error(LOG_STARTUP, "CNF", "SYNTX", "%s: %s %s", cmd, template, arg);
  } else {
    notify_printf(player, template, arg);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * cf_status_from_succfail: Return command status from succ and fail info
 */

static int cf_status_from_succfail(DbRef player, char *cmd, int success,
                                   int failure) {

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
    if (mudstate.initializing) {
      log_error(LOG_STARTUP, "CNF", "NDATA", "%s: Nothing to set", cmd);
    } else {
      notify(player, "Nothing to set");
    }
  }
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_int: Set integer parameter.
 */

static int cf_int(int *vp, char *str, long extra, DbRef player, char *cmd) {
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

NameTable bool_names[] = {{(char *)"true", 1, 0, 1},
                          {(char *)"false", 1, 0, 0},
                          {(char *)"yes", 1, 0, 1},
                          {(char *)"no", 1, 0, 0},
                          {(char *)"1", 1, 0, 1},
                          {(char *)"0", 1, 0, 0},
                          {NULL, 0, 0, 0}};

/* *INDENT-ON* */

static int cf_bool(int *vp, char *str, long extra, DbRef player, char *cmd) {
  *vp = (int)name_table_search(GOD, bool_names, str);
  if (*vp < 0)
    *vp = (long)0;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_string: Set string parameter.
 */

static int cf_string(int *vp, char *str, long extra, DbRef player, char *cmd) {
  int retval;

  /*
   * Copy the string to the buffer if it is not too big
   */

  retval = 0;
  if (strlen(str) >= (size_t)extra) {
    str[extra - 1] = '\0';
    if (mudstate.initializing) {
      log_error(LOG_STARTUP, "CNF", "NFND", "%s: String truncated", cmd);
    } else {
      notify(player, "String truncated");
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

static int cf_alias(int *vp, char *str, long extra, DbRef player, char *cmd) {
  char *alias, *orig, *p;
  int *cp = NULL;

  alias = strtok(str, " \t=,");
  orig = strtok(NULL, " \t=,");
  if (orig) {
    for (p = orig; *p; p++)
      *p = ToLower(*p);
    cp = hash_table_find(orig, (HashTable *)vp);
    if (cp == NULL) {
      for (p = orig; *p; p++)
        *p = ToUpper(*p);
      cp = hash_table_find(orig, (HashTable *)vp);
      if (cp == NULL) {
        configuration_log_not_found(player, cmd, "Entry", orig);
        return -1;
      }
    }
  }

  if (cp == NULL) {
    return -1;
  }

  hash_table_add(alias, cp, (HashTable *)vp);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_flagalias: define a flag alias.
 */

static int cf_flagalias(int *vp, char *str, long extra, DbRef player,
                        char *cmd) {
  char *alias, *orig;
  int *cp, success;

  success = 0;
  alias = strtok(str, " \t=,");
  orig = strtok(NULL, " \t=,");

  cp = hash_table_find(orig, &mudstate.flags_htab);
  if (cp != NULL) {
    hash_table_add(alias, cp, &mudstate.flags_htab);
    success++;
  }
  if (!success)
    configuration_log_not_found(player, cmd, "Flag", orig);
  return ((success > 0) ? 0 : -1);
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_modify_bits: set or clear bits in a flag word from a
 * namelist.
 */
int configuration_modify_bits(int *vp, char *str, long extra, DbRef player,
                              char *cmd) {
  char *sp;
  int f, negate, success, failure;

  /*
   * Walk through the tokens
   */

  success = failure = 0;
  sp = strtok(str, " \t");
  while (sp != NULL) {

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

    f = name_table_search(GOD, (NameTable *)extra, sp);
    if (f > 0) {
      if (negate)
        *vp &= ~f;
      else
        *vp |= f;
      success++;
    } else {
      configuration_log_not_found(player, cmd, "Entry", sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok(NULL, " \t");
  }
  return cf_status_from_succfail(player, cmd, success, failure);
}

/*
 * ---------------------------------------------------------------------------
 * * cf_set_flags: Clear flag word and then set from a flags htab.
 */

static int cf_set_flags(int *vp, char *str, long extra, DbRef player,
                        char *cmd) {
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

  while (sp != NULL) {

    /*
     * Set the appropriate bit
     */

    fp = (FLAGENT *)hash_table_find(sp, &mudstate.flags_htab);
    if (fp != NULL) {
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
      configuration_log_not_found(player, cmd, "Entry", sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok(NULL, " \t");
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

static int cf_badname(int *vp, char *str, long extra, DbRef player, char *cmd) {
  if (extra)
    badname_remove(str);
  else
    badname_add(str);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_site: Update site information
 */

static int cf_site(long **vp, char *str, long extra, DbRef player, char *cmd) {
  SiteData *site, *last, *head;
  char *addr_txt, *mask_txt;
  struct in_addr addr_num, mask_num;

  addr_txt = strtok(str, " \t=,");
  mask_txt = NULL;
  if (addr_txt)
    mask_txt = strtok(NULL, " \t=,");
  if (!addr_txt || !*addr_txt || !mask_txt || !*mask_txt) {
    configuration_log_syntax(player, cmd, "Missing host address or mask.",
                             (char *)"");
    return -1;
  }

  addr_num.s_addr = inet_addr(addr_txt);
  mask_num.s_addr = inet_addr(mask_txt);

  if (addr_num.s_addr == INADDR_NONE) {
    configuration_log_syntax(player, cmd, "Bad host address: ", addr_txt);
    return -1;
  }
  head = (SiteData *)*vp;
  /*
   * Parse the access entry and allocate space for it
   */

  site = (SiteData *)malloc(sizeof(SiteData));

  /*
   * Initialize the site entry
   */

  site->address.s_addr = addr_num.s_addr;
  site->mask.s_addr = mask_num.s_addr;
  site->flag = extra;
  site->next = NULL;

  /*
   * Link in the entry.  Link it at the start if not initializing, at *
   *
   * *  * *  * *  * *  * * the end if initializing.  This is so that
   * entries  * in * the * config * * * file are processed as you would
   * think they * * would be, * while * entries * * made while running
   * are processed * * first.
   */

  if (mudstate.initializing) {
    if (head == NULL) {
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

static int cf_cf_access(int *vp, char *str, long extra, DbRef player,
                        char *cmd) {
  CONF *tp;
  char *ap;

  for (ap = str; *ap && !isspace(*ap); ap++)
    ;
  if (*ap)
    *ap++ = '\0';

  for (tp = conftable; tp->pname; tp++) {
    if (!strcmp(tp->pname, str)) {
      return (configuration_modify_bits(&tp->flags, ap, extra, player, cmd));
    }
  }
  configuration_log_not_found(player, cmd, "Config directive", str);
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_include: Read another config file.  Only valid during startup.
 */

static int cf_include(int *vp, char *str, long extra, DbRef player, char *cmd) {
  FILE *fp;
  char *cp, *ap, *zp, *buf;

  extern int configuration_set(char *, char *, DbRef);

  if (!mudstate.initializing)
    return -1;

  fp = fopen(str, "r");
  if (fp == NULL) {
    configuration_log_not_found(player, cmd, "Config file", str);
    return -1;
  }
  buf = alloc_lbuf("cf_include");
  while (1) {
    if (!fgets(buf, LBUF_SIZE, fp))
      break;
    cp = buf;
    if (!cp || !*cp || *cp == '#' || *cp == '\n')
      continue;

    /*
     * Not a comment line or an empty one. Strip off the NL and any
     * characters following it. Then, split the line into the command
     * and argument portions (separated by a space). Also, trim off the
     * trailing comment, if any (delimited by #)
     */

    for (cp = buf; *cp && *cp != '\n'; cp++)
      ;
    *cp = '\0'; /* strip \n */

    for (cp = buf; *cp && isspace(*cp); cp++)
      ;              /* strip spaces */
    if (*cp == '\0') /* skip line if nothing left */
      continue;

    for (ap = cp; *ap && !isspace(*ap); ap++)
      ; /* skip over command */
    if (*ap)
      *ap++ = '\0'; /* trim command */

    for (; *ap && isspace(*ap); ap++)
      ; /* skip spaces */

    for (zp = ap; *zp && (*zp != '#'); zp++)
      ; /* find comment */

    if (*zp)
      *zp = '\0'; /* zap comment */

    for (zp = zp - 1; zp >= ap && isspace(*zp); zp--)
      *zp = '\0'; /* zap trailing spaces */

    configuration_set(cp, ap, player);
  }
  if (ferror(fp))
    fprintf(stderr, "Error reading config file: %s\n", strerror(errno));
  free_lbuf(buf);
  fclose(fp);
  return 0;
}

/* ---------------------------------------------------------------------------
 * conftable: Table for parsing the configuration file.
 */

CONF conftable[] = {
    {(char *)"access", cf_access, CA_GOD, NULL, (long)access_nametab},
    {(char *)"alias", cf_cmd_alias, CA_GOD, (int *)&mudstate.command_htab, 0},
    {(char *)"allow_unloggedwho", cf_int, CA_GOD, &mudconf.allow_unloggedwho,
     0},
    {(char *)"attr_access", cf_attr_access, CA_GOD, NULL,
     (long)attraccess_nametab},
    {(char *)"attr_alias", cf_alias, CA_GOD, (int *)&mudstate.attr_name_htab,
     0},
    {(char *)"attr_cmd_access", cf_acmd_access, CA_GOD, NULL,
     (long)access_nametab},
    {(char *)"bad_name", cf_badname, CA_GOD, NULL, 0},
    {(char *)"badsite_file", cf_string, CA_DISABLED, (void *)mudconf.site_file,
     32},
    {(char *)"btech_explode_reactor", cf_int, CA_GOD,
     &mudconf.btech_explode_reactor, 0},
    {(char *)"btech_explode_time", cf_int, CA_GOD, &mudconf.btech_explode_time,
     0},
    {(char *)"btech_explode_ammo", cf_int, CA_GOD, &mudconf.btech_explode_ammo,
     0},
    {(char *)"btech_explode_stop", cf_int, CA_GOD, &mudconf.btech_explode_stop,
     0},
    {(char *)"btech_parts", cf_int, CA_GOD, &mudconf.btech_parts, 0},
    {(char *)"btech_ic", cf_int, CA_GOD, &mudconf.btech_ic, 0},
    {(char *)"btech_afterlife_dbref", cf_int, CA_GOD, &mudconf.afterlife_dbref,
     0},
    {(char *)"btech_vcrit", cf_int, CA_GOD, &mudconf.btech_vcrit, 0},
    {(char *)"btech_dynspeed", cf_int, CA_GOD, &mudconf.btech_dynspeed, 0},
    {(char *)"btech_slowdown", cf_int, CA_GOD, &mudconf.btech_slowdown, 0},
    {(char *)"btech_fasaturn", cf_int, CA_GOD, &mudconf.btech_fasaturn, 0},
    {(char *)"btech_fasacrit", cf_int, CA_GOD, &mudconf.btech_fasacrit, 0},
    {(char *)"btech_fasaadvvtolcrit", cf_int, CA_GOD,
     &mudconf.btech_fasaadvvtolcrit, 0},
    {(char *)"btech_fasaadvvhlcrit", cf_int, CA_GOD,
     &mudconf.btech_fasaadvvhlcrit, 0},
    {(char *)"btech_fasaadvvhlfire", cf_int, CA_GOD,
     &mudconf.btech_fasaadvvhlfire, 0},
    {(char *)"btech_divrotordamage", cf_int, CA_GOD,
     &mudconf.btech_divrotordamage, 0},
    {(char *)"btech_moddamagewithrange", cf_int, CA_GOD,
     &mudconf.btech_moddamagewithrange, 0},
    {(char *)"btech_moddamagewithwoods", cf_int, CA_GOD,
     &mudconf.btech_moddamagewithwoods, 0},
    {(char *)"btech_hotloadaddshalfbthmod", cf_int, CA_GOD,
     &mudconf.btech_hotloadaddshalfbthmod, 0},
    {(char *)"btech_nofusionvtolfuel", cf_int, CA_GOD,
     &mudconf.btech_nofusionvtolfuel, 0},
    {(char *)"btech_tankfriendly", cf_int, CA_GOD, &mudconf.btech_tankfriendly,
     0},
    {(char *)"btech_newcharge", cf_int, CA_GOD, &mudconf.btech_newcharge, 0},
    {(char *)"btech_tl3_charge", cf_int, CA_GOD, &mudconf.btech_tl3_charge, 0},
    {(char *)"btech_newterrain", cf_int, CA_GOD, &mudconf.btech_newterrain, 0},
    {(char *)"btech_xploss", cf_int, CA_GOD, &mudconf.btech_xploss, 0},
    {(char *)"btech_critlevel", cf_int, CA_GOD, &mudconf.btech_critlevel, 0},
    {(char *)"btech_tankshield", cf_int, CA_GOD, &mudconf.btech_tankshield, 0},
    {(char *)"btech_newstagger", cf_int, CA_GOD, &mudconf.btech_newstagger, 0},
    {(char *)"btech_newstaggertons", cf_int, CA_GOD,
     &mudconf.btech_newstaggertons, 0},
    {(char *)"btech_newstaggertime", cf_int, CA_GOD,
     &mudconf.btech_newstaggertime, 0},
    {(char *)"namechange_days", cf_int, CA_GOD, &mudconf.namechange_days, 0},
    {(char *)"allow_chanlurking", cf_int, CA_GOD, &mudconf.allow_chanlurking,
     0},
    {(char *)"btech_skidcliff", cf_int, CA_GOD, &mudconf.btech_skidcliff, 0},
    {(char *)"btech_xp_bthmod", cf_int, CA_GOD, &mudconf.btech_xp_bthmod, 0},
    {(char *)"btech_xp_missilemod", cf_int, CA_GOD,
     &mudconf.btech_xp_missilemod, 0},
    {(char *)"btech_xp_ammomod", cf_int, CA_GOD, &mudconf.btech_xp_ammomod, 0},
    {(char *)"btech_defaultweapdam", cf_int, CA_GOD,
     &mudconf.btech_defaultweapdam, 0},
    {(char *)"btech_xp_modifier", cf_int, CA_GOD, &mudconf.btech_xp_modifier,
     0},
    {(char *)"btech_defaultweapbv", cf_int, CA_GOD,
     &mudconf.btech_defaultweapbv, 0},
    {(char *)"btech_xp_usePilotBVMod", cf_int, CA_GOD,
     &mudconf.btech_xp_usePilotBVMod, 0},
    {(char *)"btech_oldxpsystem", cf_int, CA_GOD, &mudconf.btech_oldxpsystem,
     0},
    {(char *)"btech_xp_vrtmod", cf_int, CA_GOD, &mudconf.btech_xp_vrtmod, 0},
    {(char *)"btech_extendedmovemod", cf_int, CA_GOD,
     &mudconf.btech_extendedmovemod, 0},
    {(char *)"btech_stacking", cf_int, CA_GOD, &mudconf.btech_stacking, 0},
    {(char *)"btech_stackdamage", cf_int, CA_GOD, &mudconf.btech_stackdamage,
     0},
    {(char *)"btech_mw_losmap", cf_int, CA_GOD, &mudconf.btech_mw_losmap, 0},
    {(char *)"btech_exile_stun_code", cf_int, CA_GOD,
     &mudconf.btech_exile_stun_code, 0},
    {(char *)"btech_roll_on_backwalk", cf_int, CA_GOD,
     &mudconf.btech_roll_on_backwalk, 0},
    {(char *)"btech_usedmechstore", cf_int, CA_GOD,
     &mudconf.btech_usedmechstore, 0},
    {(char *)"btech_ooc_comsys", cf_int, CA_GOD, &mudconf.btech_ooc_comsys, 0},
    {(char *)"btech_idf_requires_spotter", cf_int, CA_GOD,
     &mudconf.btech_idf_requires_spotter, 0},
    {(char *)"btech_perunit_xpmod", cf_int, CA_GOD,
     &mudconf.btech_perunit_xpmod, 0},
    {(char *)"btech_tsm_tow_bonus", cf_int, CA_GOD,
     &mudconf.btech_tsm_tow_bonus, 0},
    {(char *)"btech_heatcutoff", cf_int, CA_GOD, &mudconf.btech_heatcutoff, 0},
    {(char *)"btech_cost_debug", cf_int, CA_GOD, &mudconf.btech_cost_debug, 0},
    {(char *)"btech_noisy_xpgain", cf_int, CA_GOD, &mudconf.btech_noisy_xpgain,
     0},
    {(char *)"btech_xpgain_cap", cf_int, CA_GOD, &mudconf.btech_xpgain_cap, 0},
    {(char *)"btech_transported_unit_death", cf_int, CA_GOD,
     &mudconf.btech_transported_unit_death, 0},
    {(char *)"btech_mwpickup_action", cf_int, CA_GOD,
     &mudconf.btech_mwpickup_action, 0},
    {(char *)"btech_standcareful", cf_int, CA_GOD, &mudconf.btech_standcareful,
     0},
    {(char *)"btech_maxtechtime", cf_int, CA_GOD, &mudconf.btech_maxtechtime,
     0},
    {(char *)"btech_sprint_bth", cf_int, CA_GOD, &mudconf.btech_sprint_bth, 0},
    {(char *)"btech_tsm_sprint_bonus", cf_int, CA_GOD,
     &mudconf.btech_tsm_sprint_bonus, 0},
    {(char *)"btech_vtol_ice_causes_fire", cf_int, CA_GOD,
     &mudconf.btech_vtol_ice_causes_fire, 0},
    {(char *)"btech_glancing_blows", cf_int, CA_GOD,
     &mudconf.btech_glancing_blows, 0},
    {(char *)"btech_inferno_penalty", cf_int, CA_GOD,
     &mudconf.btech_inferno_penalty, 0},
    {(char *)"btech_blzmapmode", cf_int, CA_GOD, &mudconf.btech_blzmapmode, 0},
    {(char *)"btech_extended_piloting", cf_int, CA_GOD,
     &mudconf.btech_extended_piloting, 0},
    {(char *)"btech_extended_gunnery", cf_int, CA_GOD,
     &mudconf.btech_extended_gunnery, 0},
    {(char *)"btech_xploss_for_mw", cf_int, CA_GOD,
     &mudconf.btech_xploss_for_mw, 0},
    {(char *)"btech_variable_techtime", cf_int, CA_GOD,
     &mudconf.btech_variable_techtime, 0},
    {(char *)"btech_techtime_mod", cf_int, CA_GOD, &mudconf.btech_techtime_mod,
     0},
    {(char *)"btech_statengine_obj", cf_int, CA_GOD,
     &mudconf.btech_statengine_obj, 0},
#ifdef BT_FREETECHTIME
    {(char *)"btech_freetechtime", cf_int, CA_GOD, &mudconf.btech_freetechtime,
     0},
#endif
#ifdef BT_COMPLEXREPAIRS
    {(char *)"btech_complexrepair", cf_int, CA_GOD,
     &mudconf.btech_complexrepair, 0},
#endif
    {(char *)"btech_seismic_see_stopped", cf_int, CA_GOD,
     &mudconf.btech_seismic_see_stopped, 0},
    {(char *)"btech_limitedrepairs", cf_int, CA_GOD,
     &mudconf.btech_limitedrepairs, 0},
    {(char *)"btech_stackpole", cf_int, CA_GOD, &mudconf.btech_stackpole, 0},
    {(char *)"btech_phys_use_pskill", cf_int, CA_GOD,
     &mudconf.btech_phys_use_pskill, 0},
    {(char *)"btech_erange", cf_int, CA_GOD, &mudconf.btech_erange, 0},
    {(char *)"btech_hit_arcs", cf_int, CA_GOD, &mudconf.btech_hit_arcs, 0},
    {(char *)"btech_dig_only_fs", cf_int, CA_GOD, &mudconf.btech_dig_only_fs,
     0},
    {(char *)"btech_digbonus", cf_int, CA_GOD, &mudconf.btech_digbonus, 0},
    {(char *)"cache_depth", cf_int, CA_DISABLED, &mudconf.cache_depth, 0},
    {(char *)"cache_names", cf_bool, CA_DISABLED, &mudconf.cache_names, 0},
    {(char *)"cache_trim", cf_bool, CA_GOD, &mudconf.cache_trim, 0},
    {(char *)"cache_width", cf_int, CA_DISABLED, &mudconf.cache_width, 0},
    {(char *)"check_interval", cf_int, CA_GOD, &mudconf.check_interval, 0},
    {(char *)"check_offset", cf_int, CA_GOD, &mudconf.check_offset, 0},
    {(char *)"command_quota_increment", cf_int, CA_GOD, &mudconf.cmd_quota_incr,
     0},
    {(char *)"command_quota_max", cf_int, CA_GOD, &mudconf.cmd_quota_max, 0},
    {(char *)"concentrator_port", cf_int, CA_DISABLED, &mudconf.conc_port, 0},
    {(char *)"config_access", cf_cf_access, CA_GOD, NULL, (long)access_nametab},
    {(char *)"conn_timeout", cf_int, CA_GOD, &mudconf.conn_timeout, 0},
    {(char *)"connect_dir", cf_string, CA_DISABLED, (void *)mudconf.conn_dir,
     32},
    {(char *)"connect_file", cf_string, CA_DISABLED, (void *)mudconf.conn_file,
     32},
    {(char *)"connect_reg_file", cf_string, CA_DISABLED,
     (void *)mudconf.creg_file, 32},
    {(char *)"dark_sleepers", cf_bool, CA_GOD, &mudconf.dark_sleepers, 0},
    {(char *)"default_home", cf_int, CA_GOD, &mudconf.default_home, 0},
    {(char *)"down_file", cf_string, CA_DISABLED, (void *)mudconf.down_file,
     32},
    {(char *)"down_message", cf_string, CA_GOD, (void *)mudconf.down_msg, 4096},
    {(char *)"dump_interval", cf_int, CA_GOD, &mudconf.dump_interval, 0},
    {(char *)"dump_message", cf_string, CA_GOD, (void *)mudconf.dump_msg, 128},
    {(char *)"postdump_message", cf_string, CA_GOD,
     (void *)mudconf.postdump_msg, 128},
    {(char *)"dump_offset", cf_int, CA_GOD, &mudconf.dump_offset, 0},
    {(char *)"examine_flags", cf_bool, CA_GOD, &mudconf.ex_flags, 0},
    {(char *)"examine_public_attrs", cf_bool, CA_GOD, &mudconf.exam_public, 0},
    {(char *)"exit_flags", cf_set_flags, CA_GOD, (int *)&mudconf.exit_flags, 0},
    {(char *)"events_daily_hour", cf_int, CA_GOD, &mudconf.events_daily_hour,
     0},
    {(char *)"fascist_teleport", cf_bool, CA_GOD, &mudconf.fascist_tport, 0},
    {(char *)"fixed_home_message", cf_string, CA_DISABLED,
     (void *)mudconf.fixed_home_msg, 128},
    {(char *)"fixed_tel_message", cf_string, CA_DISABLED,
     (void *)mudconf.fixed_tel_msg, 128},
    {(char *)"flag_alias", cf_flagalias, CA_GOD, NULL, 0},
    {(char *)"forbid_site", cf_site, CA_GOD, (int *)&mudstate.access_list,
     H_FORBIDDEN},
    {(char *)"fork_dump", cf_bool, CA_GOD, &mudconf.fork_dump, 0},
    {(char *)"fork_vfork", cf_bool, CA_GOD, &mudconf.fork_vfork, 0},
    {(char *)"full_file", cf_string, CA_DISABLED, (void *)mudconf.full_file,
     32},
    {(char *)"full_message", cf_string, CA_GOD, (void *)mudconf.full_msg, 4096},
    {(char *)"function_access", cf_func_access, CA_GOD, NULL,
     (long)access_nametab},
    {(char *)"function_alias", cf_alias, CA_GOD, (int *)&mudstate.func_htab, 0},
    {(char *)"function_invocation_limit", cf_int, CA_GOD,
     &mudconf.func_invk_lim, 0},
    {(char *)"function_recursion_limit", cf_int, CA_GOD, &mudconf.func_nest_lim,
     0},
    {(char *)"game_database", cf_string, CA_DISABLED, (void *)mudconf.gamedb,
     128},
    {(char *)"good_name", cf_badname, CA_GOD, NULL, 1},
    {(char *)"have_specials", cf_bool, CA_DISABLED, &mudconf.have_specials, 0},
    {(char *)"have_comsys", cf_bool, CA_DISABLED, &mudconf.have_comsys, 0},
    {(char *)"have_macros", cf_bool, CA_DISABLED, &mudconf.have_macros, 0},
    {(char *)"have_zones", cf_bool, CA_DISABLED, &mudconf.have_zones, 0},
    {(char *)"help_file", cf_string, CA_DISABLED, (void *)mudconf.help_file,
     32},
    {(char *)"help_index", cf_string, CA_DISABLED, (void *)mudconf.help_indx,
     32},
    {(char *)"use_http", cf_bool, CA_DISABLED, &mudconf.use_http, 0},
    {(char *)"idle_wiz_dark", cf_bool, CA_GOD, &mudconf.idle_wiz_dark, 0},
    {(char *)"idle_interval", cf_int, CA_GOD, &mudconf.idle_interval, 0},
    {(char *)"idle_timeout", cf_int, CA_GOD, &mudconf.idle_timeout, 0},
    {(char *)"include", cf_include, CA_DISABLED, NULL, 0},
    {(char *)"indent_desc", cf_bool, CA_GOD, &mudconf.indent_desc, 0},
    {(char *)"initial_size", cf_int, CA_DISABLED, &mudconf.init_size, 0},
    {(char *)"list_access", cf_ntab_access, CA_GOD, (int *)list_names,
     (long)access_nametab},
    {(char *)"lock_recursion_limit", cf_int, CA_WIZARD, &mudconf.lock_nest_lim,
     0},
    {(char *)"lua_directory", cf_string, CA_GOD, (void *)mudconf.lua_directory,
     sizeof(mudconf.lua_directory)},
    {(char *)"lua_instruction_limit", cf_int, CA_GOD,
     &mudconf.lua_instruction_limit, 0},
    {(char *)"lua_memory_limit", cf_int, CA_GOD, &mudconf.lua_memory_limit, 0},
    {(char *)"log", configuration_modify_bits, CA_GOD, &mudconf.log_options,
     (long)logoptions_nametab},
    {(char *)"log_options", configuration_modify_bits, CA_GOD,
     &mudconf.log_info, (long)logdata_nametab},
    {(char *)"logout_cmd_access", cf_ntab_access, CA_GOD,
     (int *)logout_cmdtable, (long)access_nametab},
    {(char *)"logout_cmd_alias", cf_alias, CA_GOD,
     (int *)&mudstate.logout_cmd_htab, 0},
    {(char *)"map_database", cf_string, CA_GOD, (void *)mudconf.map_db, 128},
    {(char *)"master_room", cf_int, CA_GOD, &mudconf.master_room, 0},
    {(char *)"match_own_commands", cf_bool, CA_GOD, &mudconf.match_mine, 0},
    {(char *)"max_players", cf_int, CA_GOD, &mudconf.max_players, 0},
    {(char *)"mech_database", cf_string, CA_GOD, (void *)mudconf.mech_db, 128},
    {(char *)"mud_name", cf_string, CA_GOD, (void *)mudconf.mud_name, 32},
    {(char *)"newuser_file", cf_string, CA_DISABLED, (void *)mudconf.crea_file,
     32},
    {(char *)"notify_recursion_limit", cf_int, CA_GOD, &mudconf.ntfy_nest_lim,
     0},
    {(char *)"output_limit", cf_int, CA_GOD, &mudconf.output_limit, 0},
    {(char *)"paranoid_allocate", cf_bool, CA_GOD, &mudconf.paranoid_alloc, 0},
    {(char *)"parent_recursion_limit", cf_int, CA_GOD, &mudconf.parent_nest_lim,
     0},
    {(char *)"pemit_far_players", cf_bool, CA_GOD, &mudconf.pemit_players, 0},
    {(char *)"pemit_any_object", cf_bool, CA_GOD, &mudconf.pemit_any, 0},
    {(char *)"permit_site", cf_site, CA_GOD, (int *)&mudstate.access_list, 0},
    {(char *)"player_flags", cf_set_flags, CA_GOD, (int *)&mudconf.player_flags,
     0},
    {(char *)"player_listen", cf_bool, CA_GOD, &mudconf.player_listen, 0},
    {(char *)"player_match_own_commands", cf_bool, CA_GOD,
     &mudconf.match_mine_pl, 0},
    {(char *)"player_name_spaces", cf_bool, CA_GOD, &mudconf.name_spaces, 0},
    {(char *)"player_queue_limit", cf_int, CA_GOD, &mudconf.queuemax, 0},
    {(char *)"player_starting_home", cf_int, CA_GOD, &mudconf.start_home, 0},
    {(char *)"player_starting_room", cf_int, CA_GOD, &mudconf.start_room, 0},
    {(char *)"public_channel", cf_string, CA_DISABLED,
     (void *)mudconf.public_channel, 32},
    {(char *)"port", cf_int, CA_DISABLED, &mudconf.port, 0},
    {(char *)"public_flags", cf_bool, CA_GOD, &mudconf.pub_flags, 0},
    {(char *)"queue_active_chunk", cf_int, CA_GOD, &mudconf.active_q_chunk, 0},
    {(char *)"queue_idle_chunk", cf_int, CA_GOD, &mudconf.queue_chunk, 0},
    {(char *)"quiet_look", cf_bool, CA_GOD, &mudconf.quiet_look, 0},
    {(char *)"quit_file", cf_string, CA_DISABLED, (void *)mudconf.quit_file,
     32},
    {(char *)"read_remote_desc", cf_bool, CA_GOD, &mudconf.read_rem_desc, 0},
    {(char *)"read_remote_name", cf_bool, CA_GOD, &mudconf.read_rem_name, 0},
    {(char *)"register_create_file", cf_string, CA_DISABLED,
     (void *)mudconf.regf_file, 32},
    {(char *)"register_site", cf_site, CA_GOD, (int *)&mudstate.access_list,
     H_REGISTRATION},
    {(char *)"retry_limit", cf_int, CA_GOD, &mudconf.retry_limit, 0},
    {(char *)"robot_flags", cf_set_flags, CA_GOD, (int *)&mudconf.robot_flags,
     0},
    {(char *)"robot_speech", cf_bool, CA_GOD, &mudconf.robot_speak, 0},
    {(char *)"room_flags", cf_set_flags, CA_GOD, (int *)&mudconf.room_flags, 0},
    {(char *)"see_owned_dark", cf_bool, CA_GOD, &mudconf.see_own_dark, 0},
    {(char *)"show_unfindable_who", cf_bool, CA_GOD,
     &mudconf.show_unfindable_who, 1},
    {(char *)"space_compress", cf_bool, CA_GOD, &mudconf.space_compress, 0},
    {(char *)"stack_limit", cf_int, CA_GOD, &mudconf.stack_limit, 0},
    {(char *)"suspect_site", cf_site, CA_GOD, (int *)&mudstate.suspect_list,
     H_SUSPECT},
    {(char *)"sweep_dark", cf_bool, CA_GOD, &mudconf.sweep_dark, 0},
    {(char *)"switch_default_all", cf_bool, CA_GOD, &mudconf.switch_df_all, 0},
    {(char *)"thing_flags", cf_set_flags, CA_GOD, (int *)&mudconf.thing_flags,
     0},
    {(char *)"timeslice", cf_int, CA_GOD, &mudconf.timeslice, 0},
    {(char *)"trace_output_limit", cf_int, CA_GOD, &mudconf.trace_limit, 0},
    {(char *)"trace_topdown", cf_bool, CA_GOD, &mudconf.trace_topdown, 0},
    {(char *)"trust_site", cf_site, CA_GOD, (int *)&mudstate.suspect_list, 0},
    {(char *)"unowned_safe", cf_bool, CA_GOD, &mudconf.safe_unowned, 0},
    {(char *)"user_attr_access", configuration_modify_bits, CA_GOD,
     &mudconf.vattr_flags, (long)attraccess_nametab},
    {(char *)"wizard_help_file", cf_string, CA_DISABLED,
     (void *)mudconf.whelp_file, 32},
    {(char *)"wizard_help_index", cf_string, CA_DISABLED,
     (void *)mudconf.whelp_indx, 32},
    {(char *)"zone_recursion_limit", cf_int, CA_GOD, &mudconf.zone_nest_lim, 0},
    {(char *)"exit_parent", cf_int, CA_GOD, &mudconf.exit_parent, 0},
    {(char *)"room_parent", cf_int, CA_GOD, &mudconf.room_parent, 0},
    {(char *)"player_parent", cf_int, CA_GOD, &mudconf.player_parent, 0},
    {(char *)"player_zone", cf_int, CA_GOD, &mudconf.player_zone, 0},
    {NULL, NULL, 0, NULL, 0}};

/*
 * ---------------------------------------------------------------------------
 * * configuration_set: Set config parameter.
 */
int configuration_set(char *cp, char *ap, DbRef player) {
  CONF *tp;
  int i;
  char *buff = NULL;

  /*
   * Search the config parameter table for the command. If we find it,
   * call the handler to parse the argument.
   */

  for (tp = conftable; tp->pname; tp++) {
    if (!strcmp(tp->pname, cp)) {
      if (!mudstate.initializing && !check_access(player, tp->flags)) {
        notify(player, "Permission denied.");
        return (-1);
      }
      buff = alloc_lbuf("configuration_set");
      StringCopy(buff, ap);
      i = ((int (*)(void *, char *, long, DbRef, char *))tp->interpreter)(
          tp->loc, ap, tp->extra, player, cp);
      if (!mudstate.initializing) {
        log_error(
            LOG_CONFIGMODS, "CFG", "UPDAT",
            "%s entered config directive: %s with args '%s'. Status: %s%s%s%s",
            Name(player), cp, buff,
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

  configuration_log_not_found(player, (char *)"Set", "Config directive", cp);
  return (-1);
}

/*
 * ---------------------------------------------------------------------------
 * * do_admin: Command handler to set config params at runtime
 */
void do_admin(DbRef player, DbRef cause, int extra, char *kw, char *value) {
  int i;

  i = configuration_set(kw, value, player);
  if ((i >= 0) && !Quiet(player))
    notify(player, "Set.");
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_read: Read in config parameters from named file
 */
int configuration_read(char *fn) {
  int retval;

  StringCopy(mudconf.config_file, fn);
  mudstate.initializing = 1;
  retval = cf_include(NULL, fn, 0, 0, (char *)"init");
  mudstate.initializing = 0;

  return retval;
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_list_access: List access to config directives.
 */
void configuration_list_access(DbRef player) {
  CONF *tp;
  char *buff;

  buff = alloc_mbuf("configuration_list_access");
  for (tp = conftable; tp->pname; tp++) {
    if (God(player) || check_access(player, tp->flags)) {
      snprintf(buff, MBUF_SIZE, "%s:", tp->pname);
      name_table_list_set(player, access_nametab, tp->flags, buff, 1);
    }
  }
  free_mbuf(buff);
}

/* ----------------------------------------------------------------------
 ** fun_config: returns the option set in mudconf
 */
void fun_config(char *buff, char **bufc, DbRef player, DbRef cause,
                char *fargs[], int nfargs, char *cargs[], int ncargs) {
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
      if (cp->interpreter == (void *)cf_string) {
        safe_str((char *)cp->loc, buff, bufc);
        return;
      }

      /* [cad] bool can be returned as 0|1 or true|false softcoders should
         decide how they want it */
      if (cp->interpreter == (void *)cf_int ||
          cp->interpreter == (void *)cf_bool) {
        safe_tprintf_str(buff, bufc, "%d", *(int *)cp->loc);
        return;
      }

      /* [cad] no other idea what to do with the hashtables and stuff */
      safe_str("#-1 UNCONVERTABLE CONF TYPE", buff, bufc);
      return;
    }
  }
  safe_str("#-1", buff, bufc);
}
