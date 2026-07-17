/* configuration_toml.c - TOML configuration file loading and dispatch. */

#include "mux/server/configuration_toml.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*
 * ---------------------------------------------------------------------------
 * ConfigTomlKind: how a mapped directive's TOML value is flattened back into
 * the plain-string argument that configuration_set()'s interpreters expect.
 */

typedef enum {
  CFG_KIND_SCALAR,      /* leaf int/bool/string -> one dispatch */
  CFG_KIND_FLAG_LIST,   /* array of strings ("!"-prefix allowed) -> one
                            dispatch, space-joined */
  CFG_KIND_ALIAS_MAP,   /* table string->string -> one dispatch per key,
                            "key value" */
  CFG_KIND_ACCESS_MAP,  /* table string->(string|array) -> one dispatch per
                            key, "key perm..." */
  CFG_KIND_SITE_LIST,   /* array of {address=,mask=} tables -> one dispatch
                            per element, in order, "address mask" */
  CFG_KIND_STRING_LIST, /* array of strings -> one dispatch per element */
} ConfigTomlKind;

typedef struct {
  const char *toml_path; /* dotted path from the document root */
  const char *pname;     /* conftable directive name, passed to set_fn */
  ConfigTomlKind kind;
} ConfigTomlMapping;

/*
 * ---------------------------------------------------------------------------
 * config_toml_map: one row per directive in configuration.c's conftable[],
 * minus "include" (which has no TOML equivalent and is handled by
 * configuration_toml_load() instead). Container tables such as [battletech]
 * or [access] need no row: the walker recurses into any unmapped table.
 */

static const ConfigTomlMapping config_toml_map[] = {
    /* database */
    {"database.game_database", "game_database", CFG_KIND_SCALAR},
    {"database.mech_database", "mech_database", CFG_KIND_SCALAR},
    {"database.map_database", "map_database", CFG_KIND_SCALAR},
    {"database.dump_interval", "dump_interval", CFG_KIND_SCALAR},

    /* lua */
    {"lua.directory", "lua_directory", CFG_KIND_SCALAR},
    {"lua.instruction_limit", "lua_instruction_limit", CFG_KIND_SCALAR},
    {"lua.memory_limit", "lua_memory_limit", CFG_KIND_SCALAR},

    /* server */
    {"server.port", "port", CFG_KIND_SCALAR},
    {"server.mud_name", "mud_name", CFG_KIND_SCALAR},
    {"server.function_recursion_limit", "function_recursion_limit",
     CFG_KIND_SCALAR},
    {"server.function_invocation_limit", "function_invocation_limit",
     CFG_KIND_SCALAR},
    {"server.events_daily_hour", "events_daily_hour", CFG_KIND_SCALAR},

    /* battletech */
    {"battletech.explode_reactor", "btech_explode_reactor", CFG_KIND_SCALAR},
    {"battletech.explode_time", "btech_explode_time", CFG_KIND_SCALAR},
    {"battletech.explode_ammo", "btech_explode_ammo", CFG_KIND_SCALAR},
    {"battletech.explode_stop", "btech_explode_stop", CFG_KIND_SCALAR},
    {"battletech.parts", "btech_parts", CFG_KIND_SCALAR},
    {"battletech.ic", "btech_ic", CFG_KIND_SCALAR},
    {"battletech.afterlife_dbref", "btech_afterlife_dbref", CFG_KIND_SCALAR},
    {"battletech.vcrit", "btech_vcrit", CFG_KIND_SCALAR},
    {"battletech.dynspeed", "btech_dynspeed", CFG_KIND_SCALAR},
    {"battletech.slowdown", "btech_slowdown", CFG_KIND_SCALAR},
    {"battletech.fasaturn", "btech_fasaturn", CFG_KIND_SCALAR},
    {"battletech.fasacrit", "btech_fasacrit", CFG_KIND_SCALAR},
    {"battletech.fasaadvvtolcrit", "btech_fasaadvvtolcrit", CFG_KIND_SCALAR},
    {"battletech.fasaadvvhlcrit", "btech_fasaadvvhlcrit", CFG_KIND_SCALAR},
    {"battletech.fasaadvvhlfire", "btech_fasaadvvhlfire", CFG_KIND_SCALAR},
    {"battletech.divrotordamage", "btech_divrotordamage", CFG_KIND_SCALAR},
    {"battletech.moddamagewithrange", "btech_moddamagewithrange",
     CFG_KIND_SCALAR},
    {"battletech.moddamagewithwoods", "btech_moddamagewithwoods",
     CFG_KIND_SCALAR},
    {"battletech.hotloadaddshalfbthmod", "btech_hotloadaddshalfbthmod",
     CFG_KIND_SCALAR},
    {"battletech.nofusionvtolfuel", "btech_nofusionvtolfuel", CFG_KIND_SCALAR},
    {"battletech.tankfriendly", "btech_tankfriendly", CFG_KIND_SCALAR},
    {"battletech.newcharge", "btech_newcharge", CFG_KIND_SCALAR},
    {"battletech.tl3_charge", "btech_tl3_charge", CFG_KIND_SCALAR},
    {"battletech.newterrain", "btech_newterrain", CFG_KIND_SCALAR},
    {"battletech.xploss", "btech_xploss", CFG_KIND_SCALAR},
    {"battletech.critlevel", "btech_critlevel", CFG_KIND_SCALAR},
    {"battletech.tankshield", "btech_tankshield", CFG_KIND_SCALAR},
    {"battletech.newstagger", "btech_newstagger", CFG_KIND_SCALAR},
    {"battletech.newstaggertons", "btech_newstaggertons", CFG_KIND_SCALAR},
    {"battletech.newstaggertime", "btech_newstaggertime", CFG_KIND_SCALAR},
    {"battletech.skidcliff", "btech_skidcliff", CFG_KIND_SCALAR},
    {"battletech.extendedmovemod", "btech_extendedmovemod", CFG_KIND_SCALAR},
    {"battletech.stacking", "btech_stacking", CFG_KIND_SCALAR},
    {"battletech.stackdamage", "btech_stackdamage", CFG_KIND_SCALAR},
    {"battletech.mw_losmap", "btech_mw_losmap", CFG_KIND_SCALAR},
    {"battletech.exile_stun_code", "btech_exile_stun_code", CFG_KIND_SCALAR},
    {"battletech.roll_on_backwalk", "btech_roll_on_backwalk", CFG_KIND_SCALAR},
    {"battletech.usedmechstore", "btech_usedmechstore", CFG_KIND_SCALAR},
    {"battletech.ooc_comsys", "btech_ooc_comsys", CFG_KIND_SCALAR},
    {"battletech.idf_requires_spotter", "btech_idf_requires_spotter",
     CFG_KIND_SCALAR},
    {"battletech.tsm_tow_bonus", "btech_tsm_tow_bonus", CFG_KIND_SCALAR},
    {"battletech.heatcutoff", "btech_heatcutoff", CFG_KIND_SCALAR},
    {"battletech.cost_debug", "btech_cost_debug", CFG_KIND_SCALAR},
    {"battletech.transported_unit_death", "btech_transported_unit_death",
     CFG_KIND_SCALAR},
    {"battletech.mwpickup_action", "btech_mwpickup_action", CFG_KIND_SCALAR},
    {"battletech.standcareful", "btech_standcareful", CFG_KIND_SCALAR},
    {"battletech.maxtechtime", "btech_maxtechtime", CFG_KIND_SCALAR},
    {"battletech.sprint_bth", "btech_sprint_bth", CFG_KIND_SCALAR},
    {"battletech.tsm_sprint_bonus", "btech_tsm_sprint_bonus", CFG_KIND_SCALAR},
    {"battletech.vtol_ice_causes_fire", "btech_vtol_ice_causes_fire",
     CFG_KIND_SCALAR},
    {"battletech.glancing_blows", "btech_glancing_blows", CFG_KIND_SCALAR},
    {"battletech.inferno_penalty", "btech_inferno_penalty", CFG_KIND_SCALAR},
    {"battletech.blzmapmode", "btech_blzmapmode", CFG_KIND_SCALAR},
    {"battletech.extended_piloting", "btech_extended_piloting",
     CFG_KIND_SCALAR},
    {"battletech.extended_gunnery", "btech_extended_gunnery", CFG_KIND_SCALAR},
    {"battletech.xploss_for_mw", "btech_xploss_for_mw", CFG_KIND_SCALAR},
    {"battletech.variable_techtime", "btech_variable_techtime",
     CFG_KIND_SCALAR},
    {"battletech.techtime_mod", "btech_techtime_mod", CFG_KIND_SCALAR},
    {"battletech.statengine_obj", "btech_statengine_obj", CFG_KIND_SCALAR},
    {"battletech.freetechtime", "btech_freetechtime", CFG_KIND_SCALAR},
    {"battletech.complexrepair", "btech_complexrepair", CFG_KIND_SCALAR},
    {"battletech.seismic_see_stopped", "btech_seismic_see_stopped",
     CFG_KIND_SCALAR},
    {"battletech.limitedrepairs", "btech_limitedrepairs", CFG_KIND_SCALAR},
    {"battletech.stackpole", "btech_stackpole", CFG_KIND_SCALAR},
    {"battletech.phys_use_pskill", "btech_phys_use_pskill", CFG_KIND_SCALAR},
    {"battletech.erange", "btech_erange", CFG_KIND_SCALAR},
    {"battletech.hit_arcs", "btech_hit_arcs", CFG_KIND_SCALAR},
    {"battletech.dig_only_fs", "btech_dig_only_fs", CFG_KIND_SCALAR},
    {"battletech.digbonus", "btech_digbonus", CFG_KIND_SCALAR},

    /* battletech.xp */
    {"battletech.xp.bthmod", "btech_xp_bthmod", CFG_KIND_SCALAR},
    {"battletech.xp.missilemod", "btech_xp_missilemod", CFG_KIND_SCALAR},
    {"battletech.xp.ammomod", "btech_xp_ammomod", CFG_KIND_SCALAR},
    {"battletech.xp.defaultweapdam", "btech_defaultweapdam", CFG_KIND_SCALAR},
    {"battletech.xp.modifier", "btech_xp_modifier", CFG_KIND_SCALAR},
    {"battletech.xp.defaultweapbv", "btech_defaultweapbv", CFG_KIND_SCALAR},
    {"battletech.xp.use_pilot_bv_mod", "btech_xp_usePilotBVMod",
     CFG_KIND_SCALAR},
    {"battletech.xp.oldxpsystem", "btech_oldxpsystem", CFG_KIND_SCALAR},
    {"battletech.xp.vrtmod", "btech_xp_vrtmod", CFG_KIND_SCALAR},
    {"battletech.xp.perunit_xpmod", "btech_perunit_xpmod", CFG_KIND_SCALAR},
    {"battletech.xp.noisy_xpgain", "btech_noisy_xpgain", CFG_KIND_SCALAR},
    {"battletech.xp.xpgain_cap", "btech_xpgain_cap", CFG_KIND_SCALAR},

    /* mux (base server behavior not covered by a more specific section) */
    {"mux.badsite_file", "badsite_file", CFG_KIND_SCALAR},
    {"mux.namechange_days", "namechange_days", CFG_KIND_SCALAR},
    {"mux.allow_chanlurking", "allow_chanlurking", CFG_KIND_SCALAR},
    {"mux.cache_depth", "cache_depth", CFG_KIND_SCALAR},
    {"mux.cache_names", "cache_names", CFG_KIND_SCALAR},
    {"mux.cache_trim", "cache_trim", CFG_KIND_SCALAR},
    {"mux.cache_width", "cache_width", CFG_KIND_SCALAR},
    {"mux.check_interval", "check_interval", CFG_KIND_SCALAR},
    {"mux.check_offset", "check_offset", CFG_KIND_SCALAR},
    {"mux.command_quota_increment", "command_quota_increment", CFG_KIND_SCALAR},
    {"mux.command_quota_max", "command_quota_max", CFG_KIND_SCALAR},
    {"mux.concentrator_port", "concentrator_port", CFG_KIND_SCALAR},
    {"mux.conn_timeout", "conn_timeout", CFG_KIND_SCALAR},
    {"mux.connect_dir", "connect_dir", CFG_KIND_SCALAR},
    {"mux.connect_file", "connect_file", CFG_KIND_SCALAR},
    {"mux.connect_reg_file", "connect_reg_file", CFG_KIND_SCALAR},
    {"mux.dark_sleepers", "dark_sleepers", CFG_KIND_SCALAR},
    {"mux.default_home", "default_home", CFG_KIND_SCALAR},
    {"mux.down_file", "down_file", CFG_KIND_SCALAR},
    {"mux.down_message", "down_message", CFG_KIND_SCALAR},
    {"mux.dump_message", "dump_message", CFG_KIND_SCALAR},
    {"mux.postdump_message", "postdump_message", CFG_KIND_SCALAR},
    {"mux.dump_offset", "dump_offset", CFG_KIND_SCALAR},
    {"mux.examine_flags", "examine_flags", CFG_KIND_SCALAR},
    {"mux.examine_public_attrs", "examine_public_attrs", CFG_KIND_SCALAR},
    {"mux.fascist_teleport", "fascist_teleport", CFG_KIND_SCALAR},
    {"mux.fixed_home_message", "fixed_home_message", CFG_KIND_SCALAR},
    {"mux.fixed_tel_message", "fixed_tel_message", CFG_KIND_SCALAR},
    {"mux.fork_dump", "fork_dump", CFG_KIND_SCALAR},
    {"mux.fork_vfork", "fork_vfork", CFG_KIND_SCALAR},
    {"mux.full_file", "full_file", CFG_KIND_SCALAR},
    {"mux.full_message", "full_message", CFG_KIND_SCALAR},
    {"mux.have_specials", "have_specials", CFG_KIND_SCALAR},
    {"mux.have_comsys", "have_comsys", CFG_KIND_SCALAR},
    {"mux.have_macros", "have_macros", CFG_KIND_SCALAR},
    {"mux.have_zones", "have_zones", CFG_KIND_SCALAR},
    {"mux.help_directory", "help_directory", CFG_KIND_SCALAR},
    {"mux.use_http", "use_http", CFG_KIND_SCALAR},
    {"mux.idle_wiz_dark", "idle_wiz_dark", CFG_KIND_SCALAR},
    {"mux.idle_interval", "idle_interval", CFG_KIND_SCALAR},
    {"mux.idle_timeout", "idle_timeout", CFG_KIND_SCALAR},
    {"mux.indent_desc", "indent_desc", CFG_KIND_SCALAR},
    {"mux.initial_size", "initial_size", CFG_KIND_SCALAR},
    {"mux.lock_recursion_limit", "lock_recursion_limit", CFG_KIND_SCALAR},
    {"mux.master_room", "master_room", CFG_KIND_SCALAR},
    {"mux.match_own_commands", "match_own_commands", CFG_KIND_SCALAR},
    {"mux.max_players", "max_players", CFG_KIND_SCALAR},
    {"mux.notify_recursion_limit", "notify_recursion_limit", CFG_KIND_SCALAR},
    {"mux.output_limit", "output_limit", CFG_KIND_SCALAR},
    {"mux.paranoid_allocate", "paranoid_allocate", CFG_KIND_SCALAR},
    {"mux.parent_recursion_limit", "parent_recursion_limit", CFG_KIND_SCALAR},
    {"mux.pemit_far_players", "pemit_far_players", CFG_KIND_SCALAR},
    {"mux.pemit_any_object", "pemit_any_object", CFG_KIND_SCALAR},
    {"mux.player_listen", "player_listen", CFG_KIND_SCALAR},
    {"mux.player_match_own_commands", "player_match_own_commands",
     CFG_KIND_SCALAR},
    {"mux.player_name_spaces", "player_name_spaces", CFG_KIND_SCALAR},
    {"mux.player_queue_limit", "player_queue_limit", CFG_KIND_SCALAR},
    {"mux.player_starting_home", "player_starting_home", CFG_KIND_SCALAR},
    {"mux.player_starting_room", "player_starting_room", CFG_KIND_SCALAR},
    {"mux.public_channel", "public_channel", CFG_KIND_SCALAR},
    {"mux.public_flags", "public_flags", CFG_KIND_SCALAR},
    {"mux.queue_active_chunk", "queue_active_chunk", CFG_KIND_SCALAR},
    {"mux.queue_idle_chunk", "queue_idle_chunk", CFG_KIND_SCALAR},
    {"mux.quiet_look", "quiet_look", CFG_KIND_SCALAR},
    {"mux.quit_file", "quit_file", CFG_KIND_SCALAR},
    {"mux.read_remote_desc", "read_remote_desc", CFG_KIND_SCALAR},
    {"mux.read_remote_name", "read_remote_name", CFG_KIND_SCALAR},
    {"mux.retry_limit", "retry_limit", CFG_KIND_SCALAR},
    {"mux.robot_speech", "robot_speech", CFG_KIND_SCALAR},
    {"mux.see_owned_dark", "see_owned_dark", CFG_KIND_SCALAR},
    {"mux.show_unfindable_who", "show_unfindable_who", CFG_KIND_SCALAR},
    {"mux.space_compress", "space_compress", CFG_KIND_SCALAR},
    {"mux.stack_limit", "stack_limit", CFG_KIND_SCALAR},
    {"mux.sweep_dark", "sweep_dark", CFG_KIND_SCALAR},
    {"mux.switch_default_all", "switch_default_all", CFG_KIND_SCALAR},
    {"mux.timeslice", "timeslice", CFG_KIND_SCALAR},
    {"mux.trace_output_limit", "trace_output_limit", CFG_KIND_SCALAR},
    {"mux.trace_topdown", "trace_topdown", CFG_KIND_SCALAR},
    {"mux.unowned_safe", "unowned_safe", CFG_KIND_SCALAR},
    {"mux.zone_recursion_limit", "zone_recursion_limit", CFG_KIND_SCALAR},
    {"mux.exit_parent", "exit_parent", CFG_KIND_SCALAR},
    {"mux.room_parent", "room_parent", CFG_KIND_SCALAR},
    {"mux.player_parent", "player_parent", CFG_KIND_SCALAR},
    {"mux.player_zone", "player_zone", CFG_KIND_SCALAR},

    /* security */
    {"security.player_password_length_limit", "player_password_length_limit",
     CFG_KIND_SCALAR},
    {"security.password_hash_opslimit", "password_hash_opslimit",
     CFG_KIND_SCALAR},
    {"security.password_hash_memlimit", "password_hash_memlimit",
     CFG_KIND_SCALAR},
    {"security.login_attempt_burst", "login_attempt_burst", CFG_KIND_SCALAR},
    {"security.login_attempt_refill", "login_attempt_refill", CFG_KIND_SCALAR},
    {"security.login_hash_limit", "login_hash_limit", CFG_KIND_SCALAR},

    /* flags (FLAGSET replace-whole-set directives) */
    {"flags.player", "player_flags", CFG_KIND_FLAG_LIST},
    {"flags.exit", "exit_flags", CFG_KIND_FLAG_LIST},
    {"flags.room", "room_flags", CFG_KIND_FLAG_LIST},
    {"flags.robot", "robot_flags", CFG_KIND_FLAG_LIST},
    {"flags.thing", "thing_flags", CFG_KIND_FLAG_LIST},

    /* logging (negatable-bitmask directives) */
    {"logging.log", "log", CFG_KIND_FLAG_LIST},
    {"logging.log_options", "log_options", CFG_KIND_FLAG_LIST},

    /* access */
    {"access.commands", "access", CFG_KIND_ACCESS_MAP},
    {"access.functions", "function_access", CFG_KIND_ACCESS_MAP},
    {"access.attrs", "attr_access", CFG_KIND_ACCESS_MAP},
    {"access.attr_commands", "attr_cmd_access", CFG_KIND_ACCESS_MAP},
    {"access.lists", "list_access", CFG_KIND_ACCESS_MAP},
    {"access.config", "config_access", CFG_KIND_ACCESS_MAP},
    /* user_attr_access is a negatable bitmask, not a per-key access map,
       despite living in the same [access] section. */
    {"access.user_attrs", "user_attr_access", CFG_KIND_FLAG_LIST},

    /* aliases */
    {"aliases.commands", "alias", CFG_KIND_ALIAS_MAP},
    {"aliases.flags", "flag_alias", CFG_KIND_ALIAS_MAP},
    {"aliases.functions", "function_alias", CFG_KIND_ALIAS_MAP},
    {"aliases.attrs", "attr_alias", CFG_KIND_ALIAS_MAP},

    /* names */
    {"names.bad", "bad_name", CFG_KIND_STRING_LIST},
    {"names.good", "good_name", CFG_KIND_STRING_LIST},

    /* sites */
    {"sites.forbid", "forbid_site", CFG_KIND_SITE_LIST},
    {"sites.suspect", "suspect_site", CFG_KIND_SITE_LIST},
    {"sites.trust", "trust_site", CFG_KIND_SITE_LIST},
    {"sites.permit", "permit_site", CFG_KIND_SITE_LIST},

    {nullptr, nullptr, CFG_KIND_SCALAR},
};

static const ConfigTomlMapping *configuration_toml_map_find(const char *path) {
  const ConfigTomlMapping *m;

  for (m = config_toml_map; m->pname != nullptr; m++) {
    if (!strcmp(m->toml_path, path))
      return m;
  }
  return nullptr;
}

/*
 * ---------------------------------------------------------------------------
 * Value flattening helpers.
 */

static char *configuration_toml_join_strings(toml_datum_t array,
                                             const char *sep) {
  size_t total;
  size_t seplen;
  int i;
  char *out;

  seplen = strlen(sep);
  total = 1;
  for (i = 0; i < array.u.arr.size; i++) {
    if (array.u.arr.elem[i].type == TOML_STRING)
      total += strlen(array.u.arr.elem[i].u.s) + seplen;
  }
  out = malloc(total);
  out[0] = '\0';
  for (i = 0; i < array.u.arr.size; i++) {
    if (array.u.arr.elem[i].type != TOML_STRING)
      continue;
    if (out[0] != '\0')
      strcat(out, sep);
    strcat(out, array.u.arr.elem[i].u.s);
  }
  return out;
}

static bool configuration_toml_format_scalar(toml_datum_t datum, char *buf,
                                             size_t buf_size) {
  if (datum.type == TOML_STRING) {
    snprintf(buf, buf_size, "%s", datum.u.s);
    return true;
  }
  if (datum.type == TOML_INT64) {
    snprintf(buf, buf_size, "%lld", (long long)datum.u.int64);
    return true;
  }
  if (datum.type == TOML_BOOLEAN) {
    snprintf(buf, buf_size, "%s", datum.u.boolean ? "true" : "false");
    return true;
  }
  return false;
}

static void configuration_toml_dispatch(const ConfigTomlMapping *m,
                                        toml_datum_t value,
                                        ConfigDirectiveSetFn set_fn, void *ctx,
                                        const char *path) {
  char scalar_buf[512];
  char *joined;
  char *args;
  int i;

  switch (m->kind) {
  case CFG_KIND_SCALAR:
    if (!configuration_toml_format_scalar(value, scalar_buf,
                                          sizeof(scalar_buf))) {
      fprintf(stderr, "configuration_toml: '%s' expected a scalar value\n",
              path);
      return;
    }
    set_fn(m->pname, scalar_buf, ctx);
    return;

  case CFG_KIND_FLAG_LIST:
    if (value.type != TOML_ARRAY) {
      fprintf(stderr, "configuration_toml: '%s' expected an array\n", path);
      return;
    }
    joined = configuration_toml_join_strings(value, " ");
    set_fn(m->pname, joined, ctx);
    free(joined);
    return;

  case CFG_KIND_STRING_LIST:
    if (value.type != TOML_ARRAY) {
      fprintf(stderr, "configuration_toml: '%s' expected an array\n", path);
      return;
    }
    for (i = 0; i < value.u.arr.size; i++) {
      if (value.u.arr.elem[i].type != TOML_STRING)
        continue;
      set_fn(m->pname, value.u.arr.elem[i].u.s, ctx);
    }
    return;

  case CFG_KIND_ALIAS_MAP:
    if (value.type != TOML_TABLE) {
      fprintf(stderr, "configuration_toml: '%s' expected a table\n", path);
      return;
    }
    for (i = 0; i < value.u.tab.size; i++) {
      size_t len;

      if (value.u.tab.value[i].type != TOML_STRING)
        continue;
      len =
          strlen(value.u.tab.key[i]) + 1 + strlen(value.u.tab.value[i].u.s) + 1;
      args = malloc(len);
      snprintf(args, len, "%s %s", value.u.tab.key[i],
               value.u.tab.value[i].u.s);
      set_fn(m->pname, args, ctx);
      free(args);
    }
    return;

  case CFG_KIND_ACCESS_MAP:
    if (value.type != TOML_TABLE) {
      fprintf(stderr, "configuration_toml: '%s' expected a table\n", path);
      return;
    }
    for (i = 0; i < value.u.tab.size; i++) {
      toml_datum_t perm_value = value.u.tab.value[i];
      const char *perms = nullptr;
      char *owned = nullptr;
      size_t len;

      if (perm_value.type == TOML_STRING) {
        perms = perm_value.u.s;
      } else if (perm_value.type == TOML_ARRAY) {
        owned = configuration_toml_join_strings(perm_value, " ");
        perms = owned;
      } else {
        fprintf(stderr,
                "configuration_toml: '%s.%s' expected a string or array\n",
                path, value.u.tab.key[i]);
        continue;
      }
      len = strlen(value.u.tab.key[i]) + 1 + strlen(perms) + 1;
      args = malloc(len);
      snprintf(args, len, "%s %s", value.u.tab.key[i], perms);
      set_fn(m->pname, args, ctx);
      free(args);
      free(owned);
    }
    return;

  case CFG_KIND_SITE_LIST:
    if (value.type != TOML_ARRAY) {
      fprintf(stderr, "configuration_toml: '%s' expected an array\n", path);
      return;
    }
    for (i = 0; i < value.u.arr.size; i++) {
      toml_datum_t entry = value.u.arr.elem[i];
      toml_datum_t address;
      toml_datum_t mask;
      size_t len;

      if (entry.type != TOML_TABLE) {
        fprintf(stderr, "configuration_toml: '%s[%d]' expected a table\n", path,
                i);
        continue;
      }
      address = toml_get(entry, "address");
      mask = toml_get(entry, "mask");
      if (address.type != TOML_STRING || mask.type != TOML_STRING) {
        fprintf(stderr,
                "configuration_toml: '%s[%d]' requires string 'address' and "
                "'mask'\n",
                path, i);
        continue;
      }
      len = strlen(address.u.s) + 1 + strlen(mask.u.s) + 1;
      args = malloc(len);
      snprintf(args, len, "%s %s", address.u.s, mask.u.s);
      set_fn(m->pname, args, ctx);
      free(args);
    }
    return;
  }
}

/*
 * ---------------------------------------------------------------------------
 * Tree walking.
 */

static void configuration_toml_walk_table(toml_datum_t table,
                                          const char *parent_path, bool is_root,
                                          ConfigDirectiveSetFn set_fn,
                                          void *ctx) {
  int i;

  for (i = 0; i < table.u.tab.size; i++) {
    const char *key = table.u.tab.key[i];
    toml_datum_t val = table.u.tab.value[i];
    char child_path[256];
    const ConfigTomlMapping *m;

    if (is_root && !strcmp(key, "include"))
      continue;

    if (parent_path[0] == '\0')
      snprintf(child_path, sizeof(child_path), "%s", key);
    else
      snprintf(child_path, sizeof(child_path), "%s.%s", parent_path, key);

    m = configuration_toml_map_find(child_path);
    if (m != nullptr) {
      configuration_toml_dispatch(m, val, set_fn, ctx, child_path);
    } else if (val.type == TOML_TABLE) {
      configuration_toml_walk_table(val, child_path, false, set_fn, ctx);
    } else {
      fprintf(stderr, "configuration_toml: unrecognized directive '%s'\n",
              child_path);
    }
  }
}

bool configuration_toml_walk(toml_datum_t root, ConfigDirectiveSetFn set_fn,
                             void *ctx) {
  if (root.type != TOML_TABLE)
    return false;
  configuration_toml_walk_table(root, "", true, set_fn, ctx);
  return true;
}

/*
 * ---------------------------------------------------------------------------
 * File loading and include resolution.
 */

constexpr int CONFIG_TOML_MAX_INCLUDE_DEPTH = 8;

static void configuration_toml_dirname(const char *path, char *out,
                                       size_t out_size) {
  const char *slash;
  size_t len;

  slash = strrchr(path, '/');
  if (slash == nullptr) {
    out[0] = '\0';
    return;
  }
  len = (size_t)(slash - path);
  if (len >= out_size)
    len = out_size - 1;
  memcpy(out, path, len);
  out[len] = '\0';
}

static void configuration_toml_resolve(const char *base_dir, const char *rel,
                                       char *out, size_t out_size) {
  if (rel[0] == '/' || base_dir[0] == '\0')
    snprintf(out, out_size, "%s", rel);
  else
    snprintf(out, out_size, "%s/%s", base_dir, rel);
}

static bool configuration_toml_load_merged(const char *path, int depth,
                                           toml_result_t *out, char *errbuf,
                                           size_t errbuf_size) {
  toml_result_t self;
  toml_datum_t include_array;
  toml_result_t acc = {0};
  bool have_acc;
  int i;
  char base_dir[512];

  if (depth > CONFIG_TOML_MAX_INCLUDE_DEPTH) {
    snprintf(errbuf, errbuf_size, "include depth exceeded while loading '%s'",
             path);
    return false;
  }

  self = toml_parse_file_ex(path);
  if (!self.ok) {
    snprintf(errbuf, errbuf_size, "%s", self.errmsg);
    toml_free(self);
    return false;
  }

  include_array = toml_get(self.toptab, "include");
  if (include_array.type != TOML_ARRAY) {
    *out = self;
    return true;
  }

  configuration_toml_dirname(path, base_dir, sizeof(base_dir));
  have_acc = false;
  for (i = 0; i < include_array.u.arr.size; i++) {
    toml_datum_t entry = include_array.u.arr.elem[i];
    char resolved[768];
    toml_result_t inc_result;

    if (entry.type != TOML_STRING) {
      snprintf(errbuf, errbuf_size,
               "'include' entries must be strings (in '%s')", path);
      if (have_acc)
        toml_free(acc);
      toml_free(self);
      return false;
    }
    configuration_toml_resolve(base_dir, entry.u.s, resolved, sizeof(resolved));
    if (!configuration_toml_load_merged(resolved, depth + 1, &inc_result,
                                        errbuf, errbuf_size)) {
      if (have_acc)
        toml_free(acc);
      toml_free(self);
      return false;
    }
    if (!have_acc) {
      acc = inc_result;
      have_acc = true;
    } else {
      toml_result_t merged = toml_merge(&acc, &inc_result);

      toml_free(acc);
      toml_free(inc_result);
      acc = merged;
    }
  }

  if (have_acc) {
    toml_result_t merged = toml_merge(&acc, &self);

    toml_free(acc);
    toml_free(self);
    *out = merged;
  } else {
    *out = self;
  }
  return true;
}

bool configuration_toml_load(const char *path, ConfigDirectiveSetFn set_fn,
                             void *ctx, char *errbuf, size_t errbuf_size) {
  toml_result_t result;

  errbuf[0] = '\0';
  if (!configuration_toml_load_merged(path, 0, &result, errbuf, errbuf_size))
    return false;
  configuration_toml_walk(result.toptab, set_fn, ctx);
  toml_free(result);
  return true;
}
