/* configuration_toml.c - TOML configuration file loading and dispatch. */

#include "mux/server/configuration_toml.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "tomlc17.h"

/*
 * ConfigTomlKind: how a mapped directive's TOML value is flattened back into
 * the plain-string argument that configuration_set()'s interpreters expect.
 */

typedef enum {
  CFG_KIND_SCALAR,      /* leaf int/bool/string -> one dispatch */
  CFG_KIND_FLAG_LIST,   /* array of strings ("!"-prefix allowed) -> one
                            dispatch, space-joined */
  CFG_KIND_ALIAS_MAP,   /* table string->string -> one dispatch per key,
                            "key value" */
  CFG_KIND_PRESET_MAP,  /* strict table string->string -> one dispatch per key,
                            "key directives" */
  CFG_KIND_ACCESS_MAP,  /* table string->(string|array) -> one dispatch per
                            key, "key perm..." */
  CFG_KIND_RGB_MAP,     /* table string->[r,g,b] -> one dispatch per key,
                            "key r g b" */
  CFG_KIND_SITE_LIST,   /* array of {address=,mask=} tables -> one dispatch
                            per element, in order, "address mask" */
  CFG_KIND_STRING_LIST, /* array of strings -> one dispatch per element */
  CFG_KIND_BOOTSTRAP_MAP,
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

static const ConfigTomlMapping CONFIG_TOML_MAP[] = {
    /* database */
    {"database.game_database", "game_database", CFG_KIND_SCALAR},
    {"database.mech_database", "mech_database", CFG_KIND_SCALAR},
    {"database.map_database", "map_database", CFG_KIND_SCALAR},
    {"database.dump_interval", "dump_interval", CFG_KIND_SCALAR},
    {"database.fork_dump", "fork_dump", CFG_KIND_SCALAR},
    {"database.dump_message", "dump_message", CFG_KIND_SCALAR},
    {"database.postdump_message", "postdump_message", CFG_KIND_SCALAR},
    {"database.bootstrap.objects", "bootstrap_object", CFG_KIND_BOOTSTRAP_MAP},

    /* lua */
    {"lua.directory", "lua_directory", CFG_KIND_SCALAR},
    {"lua.memory_limit", "lua_memory_limit", CFG_KIND_SCALAR},
    {"lua.state_value_limit", "lua_state_value_limit", CFG_KIND_SCALAR},
    {"lua.state_entry_limit", "lua_state_entry_limit", CFG_KIND_SCALAR},
    {"lua.state_object_limit", "lua_state_object_limit", CFG_KIND_SCALAR},

    /* server */
    {"server.port", "port", CFG_KIND_SCALAR},
    {"server.mud_name", "mud_name", CFG_KIND_SCALAR},
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
    {"battletech.techtime_multiplier", "btech_techtime_multiplier",
     CFG_KIND_SCALAR},
    {"battletech.statengine_obj", "btech_statengine_obj", CFG_KIND_SCALAR},
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
    {"mux.allow_chanlurking", "allow_chanlurking", CFG_KIND_SCALAR},
    {"mux.cache_depth", "cache_depth", CFG_KIND_SCALAR},
    {"mux.cache_names", "cache_names", CFG_KIND_SCALAR},
    {"mux.cache_trim", "cache_trim", CFG_KIND_SCALAR},
    {"mux.cache_width", "cache_width", CFG_KIND_SCALAR},
    {"mux.check_interval", "check_interval", CFG_KIND_SCALAR},
    {"mux.check_offset", "check_offset", CFG_KIND_SCALAR},
    {"mux.command_quota_increment", "command_quota_increment", CFG_KIND_SCALAR},
    {"mux.command_quota_max", "command_quota_max", CFG_KIND_SCALAR},
    {"mux.conn_timeout", "conn_timeout", CFG_KIND_SCALAR},
    {"mux.connect_dir", "connect_dir", CFG_KIND_SCALAR},
    {"mux.connect_file", "connect_file", CFG_KIND_SCALAR},
    {"mux.connect_reg_file", "connect_reg_file", CFG_KIND_SCALAR},
    {"mux.default_home", "default_home", CFG_KIND_SCALAR},
    {"mux.default_thing_lua_parent", "default_thing_lua_parent",
     CFG_KIND_SCALAR},
    {"mux.default_room_lua_parent", "default_room_lua_parent", CFG_KIND_SCALAR},
    {"mux.default_exit_lua_parent", "default_exit_lua_parent", CFG_KIND_SCALAR},
    {"mux.default_player_lua_parent", "default_player_lua_parent",
     CFG_KIND_SCALAR},
    {"mux.default_exit_flags", "default_exit_flags", CFG_KIND_FLAG_LIST},
    {"mux.default_player_flags", "default_player_flags", CFG_KIND_FLAG_LIST},
    {"mux.default_room_flags", "default_room_flags", CFG_KIND_FLAG_LIST},
    {"mux.default_thing_flags", "default_thing_flags", CFG_KIND_FLAG_LIST},
    {"mux.down_file", "down_file", CFG_KIND_SCALAR},
    {"mux.down_message", "down_message", CFG_KIND_SCALAR},
    {"mux.dump_offset", "dump_offset", CFG_KIND_SCALAR},
    {"mux.full_file", "full_file", CFG_KIND_SCALAR},
    {"mux.full_message", "full_message", CFG_KIND_SCALAR},
    {"mux.help_directory", "help_directory", CFG_KIND_SCALAR},
    {"mux.idle_interval", "idle_interval", CFG_KIND_SCALAR},
    {"mux.idle_timeout", "idle_timeout", CFG_KIND_SCALAR},
    {"mux.initial_size", "initial_size", CFG_KIND_SCALAR},
    {"mux.max_players", "max_players", CFG_KIND_SCALAR},
    {"mux.notify_recursion_limit", "notify_recursion_limit", CFG_KIND_SCALAR},
    {"mux.output_limit", "output_limit", CFG_KIND_SCALAR},
    {"mux.player_name_spaces", "player_name_spaces", CFG_KIND_SCALAR},
    {"mux.command_queue_limit", "command_queue_limit", CFG_KIND_SCALAR},
    {"mux.player_starting_home", "player_starting_home", CFG_KIND_SCALAR},
    {"mux.player_starting_room", "player_starting_room", CFG_KIND_SCALAR},
    {"mux.public_channel", "public_channel", CFG_KIND_SCALAR},
    {"mux.command_queue_active_chunk", "command_queue_active_chunk",
     CFG_KIND_SCALAR},
    {"mux.command_queue_idle_chunk", "command_queue_idle_chunk",
     CFG_KIND_SCALAR},
    {"mux.quit_file", "quit_file", CFG_KIND_SCALAR},
    {"mux.retry_limit", "retry_limit", CFG_KIND_SCALAR},
    {"mux.space_compress", "space_compress", CFG_KIND_SCALAR},
    {"mux.stack_limit", "stack_limit", CFG_KIND_SCALAR},
    {"mux.command_quota_interval", "command_quota_interval", CFG_KIND_SCALAR},
    {"mux.unowned_safe", "unowned_safe", CFG_KIND_SCALAR},
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

    /* logging topics */
    {"logging.topics.accounting", "accounting", CFG_KIND_SCALAR},
    {"logging.topics.all_commands", "all_commands", CFG_KIND_SCALAR},
    {"logging.topics.suspect_commands", "suspect_commands", CFG_KIND_SCALAR},
    {"logging.topics.bad_commands", "bad_commands", CFG_KIND_SCALAR},
    {"logging.topics.buffer_alloc", "buffer_alloc", CFG_KIND_SCALAR},
    {"logging.topics.bugs", "bugs", CFG_KIND_SCALAR},
    {"logging.topics.checkpoints", "checkpoints", CFG_KIND_SCALAR},
    {"logging.topics.config_changes", "config_changes", CFG_KIND_SCALAR},
    {"logging.topics.create", "create", CFG_KIND_SCALAR},
    {"logging.topics.logins", "logins", CFG_KIND_SCALAR},
    {"logging.topics.network", "network", CFG_KIND_SCALAR},
    {"logging.topics.problems", "problems", CFG_KIND_SCALAR},
    {"logging.topics.security", "security", CFG_KIND_SCALAR},
    {"logging.topics.shouts", "shouts", CFG_KIND_SCALAR},
    {"logging.topics.startup", "startup", CFG_KIND_SCALAR},
    {"logging.topics.wizard", "wizard", CFG_KIND_SCALAR},
    /* logging entry formatting */
    {"logging.log_options", "log_options", CFG_KIND_FLAG_LIST},

    /* access */
    {"access.commands", "access", CFG_KIND_ACCESS_MAP},
    {"access.lists", "list_access", CFG_KIND_ACCESS_MAP},
    {"access.config", "config_access", CFG_KIND_ACCESS_MAP},

    /* aliases */
    {"aliases.commands", "alias", CFG_KIND_ALIAS_MAP},
    {"aliases.flags", "flag_alias", CFG_KIND_ALIAS_MAP},

    /* named styled-text colors */
    {"colors", "named_color", CFG_KIND_RGB_MAP},

    /* OSC 8 preset definitions */
    {"osc8.presets", "osc8_preset", CFG_KIND_PRESET_MAP},

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

static size_t configuration_toml_array_count(toml_datum_t array) {
  return array.type == TOML_ARRAY && array.u.arr.size > 0
             ? (size_t)array.u.arr.size
             : 0;
}
static toml_datum_t configuration_toml_array_item(toml_datum_t array,
                                                  size_t index) {
  return *(const toml_datum_t *)checked_storage_at_const(
      array.u.arr.elem, configuration_toml_array_count(array),
      sizeof(*array.u.arr.elem), index);
}

static size_t configuration_toml_table_count(toml_datum_t table) {
  return table.type == TOML_TABLE && table.u.tab.size > 0
             ? (size_t)table.u.tab.size
             : 0;
}

static const char *configuration_toml_table_key(toml_datum_t table,
                                                size_t index) {
  return *(const char *const *)checked_storage_at_const(
      (const void *)table.u.tab.key, configuration_toml_table_count(table),
      sizeof(*table.u.tab.key), index);
}

static toml_datum_t configuration_toml_table_value(toml_datum_t table,
                                                   size_t index) {
  return *(const toml_datum_t *)checked_storage_at_const(
      table.u.tab.value, configuration_toml_table_count(table),
      sizeof(*table.u.tab.value), index);
}

static const ConfigTomlMapping *configuration_toml_mapping_at(size_t index) {
  return checked_storage_at_const(
      CONFIG_TOML_MAP,
      (sizeof(CONFIG_TOML_MAP) / sizeof(CONFIG_TOML_MAP[0])) - 1,
      sizeof(*CONFIG_TOML_MAP), index);
}

static const ConfigTomlMapping *configuration_toml_map_find(const char *path) {
  const size_t COUNT =
      (sizeof(CONFIG_TOML_MAP) / sizeof(CONFIG_TOML_MAP[0])) - 1;

  for (size_t index = 0; index < COUNT; index++) {
    const ConfigTomlMapping *m = configuration_toml_mapping_at(index);

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
  char *out;

  seplen = strlen(sep);
  total = 1;
  for (size_t i = 0; i < configuration_toml_array_count(array); i++) {
    toml_datum_t element = configuration_toml_array_item(array, i);

    if (element.type == TOML_STRING)
      total += strlen(element.u.s) + seplen;
  }
  out = malloc(total);
  *(char *)checked_storage_at(out, total, sizeof(char), 0) = '\0';
  for (size_t i = 0; i < configuration_toml_array_count(array); i++) {
    toml_datum_t element = configuration_toml_array_item(array, i);

    if (element.type != TOML_STRING)
      continue;
    if (*(const char *)checked_storage_at_const(out, total, sizeof(char), 0) !=
        '\0')
      strlcat(out, sep, total);
    strlcat(out, element.u.s, total);
  }
  return out;
}

static bool configuration_toml_format_scalar(toml_datum_t datum, char *buf,
                                             size_t buf_size) {
  if (datum.type == TOML_STRING) {
    (void)snprintf(buf, buf_size, "%s", datum.u.s);
    return true;
  }
  if (datum.type == TOML_INT64) {
    (void)snprintf(buf, buf_size, "%lld", (long long)datum.u.int64);
    return true;
  }
  if (datum.type == TOML_BOOLEAN) {
    (void)snprintf(buf, buf_size, "%s", datum.u.boolean ? "true" : "false");
    return true;
  }
  if (datum.type == TOML_FP64) {
    (void)snprintf(buf, buf_size, "%.17g", datum.u.fp64);
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

  switch (m->kind) {
  case CFG_KIND_SCALAR:
    if (!configuration_toml_format_scalar(value, scalar_buf,
                                          sizeof(scalar_buf))) {
      (void)fprintf(stderr,
                    "configuration_toml: '%s' expected a scalar value\n", path);
      return;
    }
    set_fn(m->pname, scalar_buf, ctx);
    return;

  case CFG_KIND_FLAG_LIST:
    if (value.type != TOML_ARRAY) {
      (void)fprintf(stderr, "configuration_toml: '%s' expected an array\n",
                    path);
      return;
    }
    joined = configuration_toml_join_strings(value, " ");
    set_fn(m->pname, joined, ctx);
    free(joined);
    return;

  case CFG_KIND_STRING_LIST:
    if (value.type != TOML_ARRAY) {
      (void)fprintf(stderr, "configuration_toml: '%s' expected an array\n",
                    path);
      return;
    }
    for (size_t i = 0; i < configuration_toml_array_count(value); i++) {
      toml_datum_t element = configuration_toml_array_item(value, i);

      if (element.type != TOML_STRING)
        continue;
      set_fn(m->pname, element.u.s, ctx);
    }
    return;

  case CFG_KIND_ALIAS_MAP:
    if (value.type != TOML_TABLE) {
      (void)fprintf(stderr, "configuration_toml: '%s' expected a table\n",
                    path);
      return;
    }
    for (size_t i = 0; i < configuration_toml_table_count(value); i++) {
      toml_datum_t element = configuration_toml_table_value(value, i);
      const char *key = configuration_toml_table_key(value, i);
      size_t len;

      if (element.type != TOML_STRING)
        continue;
      len = strlen(key) + 1 + strlen(element.u.s) + 1;
      args = malloc(len);
      (void)snprintf(args, len, "%s %s", key, element.u.s);
      set_fn(m->pname, args, ctx);
      free(args);
    }
    return;

  case CFG_KIND_PRESET_MAP:
    if (value.type != TOML_TABLE) {
      (void)fprintf(stderr, "configuration_toml: '%s' expected a table\n",
                    path);
      set_fn(m->pname, "", ctx);
      return;
    }
    for (size_t i = 0; i < configuration_toml_table_count(value); i++) {
      toml_datum_t element = configuration_toml_table_value(value, i);
      const char *key = configuration_toml_table_key(value, i);
      size_t len;

      if (element.type != TOML_STRING) {
        (void)fprintf(stderr, "configuration_toml: '%s.%s' expected a string\n",
                      path, key);
        len = strlen(key) + 2;
        args = malloc(len);
        (void)snprintf(args, len, "%s ", key);
      } else {
        len = strlen(key) + 1 + strlen(element.u.s) + 1;
        args = malloc(len);
        (void)snprintf(args, len, "%s %s", key, element.u.s);
      }
      set_fn(m->pname, args, ctx);
      free(args);
    }
    return;

  case CFG_KIND_ACCESS_MAP:
    if (value.type != TOML_TABLE) {
      (void)fprintf(stderr, "configuration_toml: '%s' expected a table\n",
                    path);
      return;
    }
    for (size_t i = 0; i < configuration_toml_table_count(value); i++) {
      toml_datum_t perm_value = configuration_toml_table_value(value, i);
      const char *key = configuration_toml_table_key(value, i);
      const char *perms = nullptr;
      char *owned = nullptr;
      size_t len;

      if (perm_value.type == TOML_STRING) {
        perms = perm_value.u.s;
      } else if (perm_value.type == TOML_ARRAY) {
        owned = configuration_toml_join_strings(perm_value, " ");
        perms = owned;
      } else {
        (void)fprintf(
            stderr, "configuration_toml: '%s.%s' expected a string or array\n",
            path, key);
        continue;
      }
      len = strlen(key) + 1 + strlen(perms) + 1;
      args = malloc(len);
      (void)snprintf(args, len, "%s %s", key, perms);
      set_fn(m->pname, args, ctx);
      free(args);
      free(owned);
    }
    return;

  case CFG_KIND_RGB_MAP:
    if (value.type != TOML_TABLE) {
      (void)fprintf(stderr, "configuration_toml: '%s' expected a table\n",
                    path);
      return;
    }
    for (size_t i = 0; i < configuration_toml_table_count(value); i++) {
      toml_datum_t rgb = configuration_toml_table_value(value, i);
      const char *key = configuration_toml_table_key(value, i);
      size_t len;
      toml_datum_t red;
      toml_datum_t green;
      toml_datum_t blue;

      if (rgb.type != TOML_ARRAY || configuration_toml_array_count(rgb) != 3) {
        (void)fprintf(
            stderr, "configuration_toml: '%s.%s' expected three RGB integers\n",
            path, key);
        continue;
      }
      red = configuration_toml_array_item(rgb, 0);
      green = configuration_toml_array_item(rgb, 1);
      blue = configuration_toml_array_item(rgb, 2);
      if (red.type != TOML_INT64 || green.type != TOML_INT64 ||
          blue.type != TOML_INT64) {
        (void)fprintf(
            stderr, "configuration_toml: '%s.%s' expected three RGB integers\n",
            path, key);
        continue;
      }
      if (red.u.int64 < 0 || red.u.int64 > 255 || green.u.int64 < 0 ||
          green.u.int64 > 255 || blue.u.int64 < 0 || blue.u.int64 > 255) {
        (void)fprintf(stderr,
                      "configuration_toml: '%s.%s' RGB values must be from 0 "
                      "through 255\n",
                      path, key);
        continue;
      }
      len = strlen(key) + 96;
      args = malloc(len);
      if (!args)
        continue;
      (void)snprintf(args, len, "%s %lld %lld %lld", key,
                     (long long)red.u.int64, (long long)green.u.int64,
                     (long long)blue.u.int64);
      set_fn(m->pname, args, ctx);
      free(args);
    }
    return;

  case CFG_KIND_SITE_LIST:
    if (value.type != TOML_ARRAY) {
      (void)fprintf(stderr, "configuration_toml: '%s' expected an array\n",
                    path);
      return;
    }
    for (size_t i = 0; i < configuration_toml_array_count(value); i++) {
      toml_datum_t entry = configuration_toml_array_item(value, i);
      toml_datum_t address;
      toml_datum_t mask;
      size_t len;

      if (entry.type != TOML_TABLE) {
        (void)fprintf(stderr,
                      "configuration_toml: '%s[%zu]' expected a table\n", path,
                      i);
        continue;
      }
      address = toml_get(entry, "address");
      mask = toml_get(entry, "mask");
      if (address.type != TOML_STRING || mask.type != TOML_STRING) {
        (void)fprintf(
            stderr,
            "configuration_toml: '%s[%zu]' requires string 'address' and "
            "'mask'\n",
            path, i);
        continue;
      }
      len = strlen(address.u.s) + 1 + strlen(mask.u.s) + 1;
      args = malloc(len);
      (void)snprintf(args, len, "%s %s", address.u.s, mask.u.s);
      set_fn(m->pname, args, ctx);
      free(args);
    }
    return;

  case CFG_KIND_BOOTSTRAP_MAP:
    if (value.type != TOML_TABLE) {
      set_fn(m->pname, "", ctx);
      return;
    }
    set_fn("bootstrap_objects_clear", "", ctx);
    for (size_t i = 0; i < configuration_toml_table_count(value); i++) {
      toml_datum_t entry = configuration_toml_table_value(value, i);
      const char *key = configuration_toml_table_key(value, i);

      if (entry.type != TOML_TABLE) {
        set_fn(m->pname, "", ctx);
        continue;
      }
      toml_datum_t type = toml_get(entry, "type");
      toml_datum_t name = toml_get(entry, "name");
      toml_datum_t wizard = toml_get(entry, "wizard");
      if (type.type != TOML_STRING || name.type != TOML_STRING ||
          (wizard.type != TOML_UNKNOWN && wizard.type != TOML_BOOLEAN)) {
        set_fn(m->pname, "", ctx);
        continue;
      }
      const char *wizard_text =
          wizard.type == TOML_BOOLEAN && wizard.u.boolean ? "true" : "false";
      size_t len = strlen(key) + strlen(type.u.s) + strlen(wizard_text) +
                   strlen(name.u.s) + 4;
      args = malloc(len);
      (void)snprintf(args, len, "%s %s %s %s", key, type.u.s, wizard_text,
                     name.u.s);
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
  for (size_t i = 0; i < configuration_toml_table_count(table); i++) {
    const char *key = configuration_toml_table_key(table, i);
    toml_datum_t val = configuration_toml_table_value(table, i);
    char child_path[256];
    const ConfigTomlMapping *m;

    if (is_root && !strcmp(key, "include"))
      continue;

    if (parent_path[0] == '\0')
      (void)snprintf(child_path, sizeof(child_path), "%s", key);
    else
      (void)snprintf(child_path, sizeof(child_path), "%s.%s", parent_path, key);

    m = configuration_toml_map_find(child_path);
    if (m != nullptr) {
      configuration_toml_dispatch(m, val, set_fn, ctx, child_path);
    } else if (val.type == TOML_TABLE) {
      configuration_toml_walk_table(val, child_path, false, set_fn, ctx);
    } else {
      (void)fprintf(stderr, "configuration_toml: unrecognized directive '%s'\n",
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

  if (out_size == 0)
    return;
  slash = strrchr(path, '/');
  if (slash == nullptr) {
    out[0] = '\0';
    return;
  }
  len = (size_t)(slash - path);
  if (len >= out_size)
    len = out_size - 1;
  memcpy(out, path, len);
  *(char *)checked_storage_at(out, out_size, sizeof(char), len) = '\0';
}

static void configuration_toml_resolve(const char *base_dir, const char *rel,
                                       char *out, size_t out_size) {
  if (rel[0] == '/' || base_dir[0] == '\0')
    (void)snprintf(out, out_size, "%s", rel);
  else
    (void)snprintf(out, out_size, "%s/%s", base_dir, rel);
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
    (void)snprintf(errbuf, errbuf_size,
                   "include depth exceeded while loading '%s'", path);
    return false;
  }

  self = toml_parse_file_ex(path);
  if (!self.ok) {
    (void)snprintf(errbuf, errbuf_size, "%s", self.errmsg);
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
    toml_datum_t entry =
        configuration_toml_array_item(include_array, (size_t)i);
    char resolved[768];
    toml_result_t inc_result;

    if (entry.type != TOML_STRING) {
      (void)snprintf(errbuf, errbuf_size,
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
