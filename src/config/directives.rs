//! C directive identities, parsers and compiled default permissions.
use crate::access::Permissions as P;
/// One directive declared by the C configuration registry.
#[derive(Clone, Copy, Debug)]
pub struct Directive {
    pub name: &'static str,
    pub parser: &'static str,
    pub permission: P,
}
/// Catalog ordered like configuration_registry.c; runtime support is classified separately.
pub const DIRECTIVES: &[Directive] = &[
    Directive {
        name: "access",
        parser: "cf_access",
        permission: P::GOD,
    },
    Directive {
        name: "alias",
        parser: "cf_cmd_alias",
        permission: P::GOD,
    },
    Directive {
        name: "bad_name",
        parser: "cf_badname",
        permission: P::GOD,
    },
    Directive {
        name: "badsite_file",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "bootstrap_objects_clear",
        parser: "cf_bootstrap_objects_clear",
        permission: P::DISABLED,
    },
    Directive {
        name: "bootstrap_object",
        parser: "cf_bootstrap_object",
        permission: P::DISABLED,
    },
    Directive {
        name: "named_color",
        parser: "cf_named_color",
        permission: P::DISABLED,
    },
    Directive {
        name: "osc8_preset",
        parser: "cf_osc8_preset",
        permission: P::DISABLED,
    },
    Directive {
        name: "btech_explode_reactor",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_explode_time",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_explode_ammo",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_explode_stop",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_parts",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_ic",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_afterlife_dbref",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_vcrit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_slowdown",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_fasaturn",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_fasaadvvtolcrit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_fasaadvvhlcrit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_fasaadvvhlfire",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_divrotordamage",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_moddamagewithrange",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_moddamagewithwoods",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_hotloadaddshalfbthmod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_nofusionvtolfuel",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_newcharge",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_tl3_charge",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_newterrain",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xploss",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_critlevel",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_newstagger",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_newstaggertons",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_newstaggertime",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "allow_chanlurking",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_skidcliff",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xp_bthmod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xp_missilemod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xp_ammomod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_defaultweapdam",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xp_modifier",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_defaultweapbv",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xp_usePilotBVMod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_oldxpsystem",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xp_vrtmod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_extendedmovemod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_stacking",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_stackdamage",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_mw_losmap",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_exile_stun_code",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_roll_on_backwalk",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_usedmechstore",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_ooc_comsys",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_idf_requires_spotter",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_perunit_xpmod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_tsm_tow_bonus",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_heatcutoff",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_cost_debug",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_allow_cargo_commands",
        parser: "cf_bool",
        permission: P::GOD,
    },
    Directive {
        name: "btech_noisy_xpgain",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xpgain_cap",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_transported_unit_death",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_mwpickup_action",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_standcareful",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_maxtechtime",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_vtol_ice_causes_fire",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_glancing_blows",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_inferno_penalty",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_blzmapmode",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_extended_piloting",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_extended_gunnery",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_xploss_for_mw",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_variable_techtime",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_techtime_mod",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_techtime_multiplier",
        parser: "cf_techtime_multiplier",
        permission: P::GOD,
    },
    Directive {
        name: "btech_statengine_obj",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_limitedrepairs",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_stackpole",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_phys_use_pskill",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_erange",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_hit_arcs",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_dig_only_fs",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "btech_digbonus",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "cache_depth",
        parser: "cf_int",
        permission: P::DISABLED,
    },
    Directive {
        name: "cache_trim",
        parser: "cf_bool",
        permission: P::GOD,
    },
    Directive {
        name: "cache_width",
        parser: "cf_int",
        permission: P::DISABLED,
    },
    Directive {
        name: "check_interval",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "check_offset",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "command_quota_increment",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "command_quota_max",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "config_access",
        parser: "cf_cf_access",
        permission: P::GOD,
    },
    Directive {
        name: "conn_timeout",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "connect_dir",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "connect_file",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "default_home",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "down_file",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "down_message",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "dump_interval",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "dump_message",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "postdump_message",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "dump_offset",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "default_exit_flags",
        parser: "cf_set_flags",
        permission: P::GOD,
    },
    Directive {
        name: "flag_alias",
        parser: "cf_flagalias",
        permission: P::GOD,
    },
    Directive {
        name: "forbid_site",
        parser: "cf_site",
        permission: P::GOD,
    },
    Directive {
        name: "fork_dump",
        parser: "cf_bool",
        permission: P::GOD,
    },
    Directive {
        name: "full_file",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "full_message",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "game_database",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "good_name",
        parser: "cf_badname",
        permission: P::GOD,
    },
    Directive {
        name: "help_directory",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "idle_interval",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "idle_timeout",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "initial_size",
        parser: "cf_int",
        permission: P::DISABLED,
    },
    Directive {
        name: "list_access",
        parser: "cf_ntab_access",
        permission: P::GOD,
    },
    Directive {
        name: "lua_directory",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "lua_memory_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "lua_error_reporting",
        parser: "cf_lua_error_reporting",
        permission: P::GOD,
    },
    Directive {
        name: "lua_state_value_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "lua_state_entry_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "lua_state_object_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "map_database",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "default_thing_lua_parent",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "default_room_lua_parent",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "default_exit_lua_parent",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "default_player_lua_parent",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "max_players",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "mech_database",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "mud_name",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "notify_recursion_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "output_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "password_hash_memlimit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "password_hash_opslimit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "permit_site",
        parser: "cf_site",
        permission: P::GOD,
    },
    Directive {
        name: "log_filter",
        parser: "cf_string",
        permission: P::GOD,
    },
    Directive {
        name: "default_player_macros",
        parser: "integer_list",
        permission: P::GOD,
    },
    Directive {
        name: "default_player_flags",
        parser: "cf_set_flags",
        permission: P::GOD,
    },
    Directive {
        name: "player_name_length_limit",
        parser: "cf_player_name_length_limit",
        permission: P::GOD,
    },
    Directive {
        name: "player_password_length_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "player_name_spaces",
        parser: "cf_bool",
        permission: P::GOD,
    },
    Directive {
        name: "command_queue_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "player_starting_home",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "player_starting_room",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "public_channel",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "port",
        parser: "cf_int",
        permission: P::DISABLED,
    },
    Directive {
        name: "command_queue_active_chunk",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "command_queue_idle_chunk",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "quit_file",
        parser: "cf_string",
        permission: P::DISABLED,
    },
    Directive {
        name: "retry_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "login_attempt_burst",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "login_attempt_refill",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "login_hash_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "default_room_flags",
        parser: "cf_set_flags",
        permission: P::GOD,
    },
    Directive {
        name: "space_compress",
        parser: "cf_bool",
        permission: P::GOD,
    },
    Directive {
        name: "stack_limit",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "suspect_site",
        parser: "cf_site",
        permission: P::GOD,
    },
    Directive {
        name: "default_thing_flags",
        parser: "cf_set_flags",
        permission: P::GOD,
    },
    Directive {
        name: "command_quota_interval",
        parser: "cf_int",
        permission: P::GOD,
    },
    Directive {
        name: "trust_site",
        parser: "cf_site",
        permission: P::GOD,
    },
    Directive {
        name: "player_zone",
        parser: "cf_int",
        permission: P::GOD,
    },
];
