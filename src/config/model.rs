//! Typed configuration values and their centralized defaults.
use super::types::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, net::IpAddr, path::PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed database config.
pub struct DatabaseConfig {
    /// Configuration value for `game_database`; defaults are centralized below.
    pub game_database: PathBuf,
    /// Configuration value for `mech_database`; defaults are centralized below.
    pub mech_database: PathBuf,
    /// Configuration value for `map_database`; defaults are centralized below.
    pub map_database: PathBuf,
    /// Configuration value for `dump_interval`; defaults are centralized below.
    pub dump_interval: i64,
    /// Configuration value for `fork_dump`; defaults are centralized below.
    pub fork_dump: bool,
    /// Configuration value for `dump_message`; defaults are centralized below.
    pub dump_message: String,
    /// Configuration value for `postdump_message`; defaults are centralized below.
    pub postdump_message: String,
    /// Deprecated compatibility value; no runtime operation uses this path.
    pub legacy_game_database: PathBuf,
    /// Configuration value for `busy_timeout_ms`; defaults are centralized below.
    pub busy_timeout_ms: u64,
    /// Most seconds the stored simulation clock may trail the running one. A heartbeat
    /// that changes nothing else stores the clock once this many seconds have passed;
    /// zero stores it only alongside other changes. Bounds the idle time a crash loses.
    pub clock_save_interval: u64,
    /// Configuration value for `bootstrap`; defaults are centralized below.
    pub bootstrap: BootstrapConfig,
}
impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            game_database: PathBuf::from("data/stompymux.db"),
            mech_database: PathBuf::from("mechs"),
            map_database: PathBuf::from("maps"),
            dump_interval: 3600,
            fork_dump: true,
            dump_message: "".into(),
            postdump_message: "".into(),
            legacy_game_database: PathBuf::from("data/stompymux.db"),
            busy_timeout_ms: 5000,
            clock_save_interval: 60,
            bootstrap: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed bootstrap config.
pub struct BootstrapConfig {
    /// Configuration value for `objects`; defaults are centralized below.
    pub objects: BTreeMap<BootstrapId, BootstrapObject>,
    /// Configuration value for `credentials_file`; defaults are centralized below.
    pub credentials_file: PathBuf,
}
impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            objects: default_bootstrap_objects(),
            credentials_file: PathBuf::from("bootstrap-credentials.txt"),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed lua config.
pub struct LuaConfig {
    /// Configuration value for `directory`; defaults are centralized below.
    pub directory: PathBuf,
    /// Configuration value for `memory_limit`; defaults are centralized below.
    pub memory_limit: usize,
    /// Configuration value for `error_reporting`; defaults are centralized below.
    pub error_reporting: ErrorReporting,
    /// Configuration value for `state_value_limit`; defaults are centralized below.
    pub state_value_limit: usize,
    /// Configuration value for `state_entry_limit`; defaults are centralized below.
    pub state_entry_limit: usize,
    /// Configuration value for `state_object_limit`; defaults are centralized below.
    pub state_object_limit: usize,
    /// Configuration value for `instruction_limit`; defaults are centralized below.
    pub instruction_limit: usize,
    /// Configuration value for `output_entry_limit`; defaults are centralized below.
    pub output_entry_limit: usize,
    /// Configuration value for `output_byte_limit`; defaults are centralized below.
    pub output_byte_limit: usize,
}
impl Default for LuaConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("lua"),
            memory_limit: 67108864,
            error_reporting: ErrorReporting::Wizards,
            state_value_limit: 65536,
            state_entry_limit: 1024,
            state_object_limit: 1048576,
            instruction_limit: 1000000,
            output_entry_limit: 1024,
            output_byte_limit: 1048576,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed server config.
pub struct ServerConfig {
    /// Configuration value for `port`; defaults are centralized below.
    pub port: u16,
    /// Configuration value for `mud_name`; defaults are centralized below.
    pub mud_name: String,
    /// Configuration value for `listen_address`; defaults are centralized below.
    pub listen_address: IpAddr,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 6250,
            mud_name: "StompyMUX".into(),
            listen_address: IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed battle tech config.
pub struct BattleTechConfig {
    /// Configuration value for `explode_reactor`; defaults are centralized below.
    pub explode_reactor: i64,
    /// Configuration value for `explode_time`; defaults are centralized below.
    pub explode_time: i64,
    /// Configuration value for `explode_ammo`; defaults are centralized below.
    pub explode_ammo: i64,
    /// Configuration value for `explode_stop`; defaults are centralized below.
    pub explode_stop: i64,
    /// Configuration value for `parts`; defaults are centralized below.
    pub parts: i64,
    /// Configuration value for `ic`; defaults are centralized below.
    pub ic: i64,
    /// Configuration value for `afterlife_dbref`; defaults are centralized below.
    pub afterlife_dbref: i64,
    /// Configuration value for `vcrit`; defaults are centralized below.
    pub vcrit: i64,
    /// Configuration value for `slowdown`; defaults are centralized below.
    pub slowdown: i64,
    /// Configuration value for `fasaturn`; defaults are centralized below.
    pub fasaturn: i64,
    /// Configuration value for `fasaadvvtolcrit`; defaults are centralized below.
    pub fasaadvvtolcrit: i64,
    /// Configuration value for `fasaadvvhlcrit`; defaults are centralized below.
    pub fasaadvvhlcrit: i64,
    /// Configuration value for `fasaadvvhlfire`; defaults are centralized below.
    pub fasaadvvhlfire: i64,
    /// Configuration value for `divrotordamage`; defaults are centralized below.
    pub divrotordamage: i64,
    /// Configuration value for `moddamagewithrange`; defaults are centralized below.
    pub moddamagewithrange: i64,
    /// Configuration value for `moddamagewithwoods`; defaults are centralized below.
    pub moddamagewithwoods: i64,
    /// Configuration value for `hotloadaddshalfbthmod`; defaults are centralized below.
    pub hotloadaddshalfbthmod: i64,
    /// Configuration value for `nofusionvtolfuel`; defaults are centralized below.
    pub nofusionvtolfuel: i64,
    /// Configuration value for `newcharge`; defaults are centralized below.
    pub newcharge: i64,
    /// Configuration value for `tl3_charge`; defaults are centralized below.
    pub tl3_charge: i64,
    /// Configuration value for `newterrain`; defaults are centralized below.
    pub newterrain: i64,
    /// Configuration value for `xploss`; defaults are centralized below.
    pub xploss: i64,
    /// Configuration value for `critlevel`; defaults are centralized below.
    pub critlevel: i64,
    /// Configuration value for `newstagger`; defaults are centralized below.
    pub newstagger: i64,
    /// Configuration value for `newstaggertons`; defaults are centralized below.
    pub newstaggertons: i64,
    /// Configuration value for `newstaggertime`; defaults are centralized below.
    pub newstaggertime: i64,
    /// Configuration value for `skidcliff`; defaults are centralized below.
    pub skidcliff: i64,
    /// Configuration value for `extendedmovemod`; defaults are centralized below.
    pub extendedmovemod: i64,
    /// Configuration value for `stacking`; defaults are centralized below.
    pub stacking: i64,
    /// Configuration value for `stackdamage`; defaults are centralized below.
    pub stackdamage: i64,
    /// Configuration value for `mw_losmap`; defaults are centralized below.
    pub mw_losmap: i64,
    /// Configuration value for `exile_stun_code`; defaults are centralized below.
    pub exile_stun_code: i64,
    /// Configuration value for `roll_on_backwalk`; defaults are centralized below.
    pub roll_on_backwalk: i64,
    /// Configuration value for `usedmechstore`; defaults are centralized below.
    pub usedmechstore: i64,
    /// Configuration value for `ooc_comsys`; defaults are centralized below.
    pub ooc_comsys: i64,
    /// Configuration value for `idf_requires_spotter`; defaults are centralized below.
    pub idf_requires_spotter: i64,
    /// Configuration value for `tsm_tow_bonus`; defaults are centralized below.
    pub tsm_tow_bonus: i64,
    /// Configuration value for `heatcutoff`; defaults are centralized below.
    pub heatcutoff: i64,
    /// Configuration value for `cost_debug`; defaults are centralized below.
    pub cost_debug: i64,
    /// Configuration value for `allow_cargo_commands`; defaults are centralized below.
    pub allow_cargo_commands: bool,
    /// Configuration value for `transported_unit_death`; defaults are centralized below.
    pub transported_unit_death: i64,
    /// Configuration value for `mwpickup_action`; defaults are centralized below.
    pub mwpickup_action: i64,
    /// Configuration value for `standcareful`; defaults are centralized below.
    pub standcareful: i64,
    /// Configuration value for `maxtechtime`; defaults are centralized below.
    pub maxtechtime: i64,
    /// Configuration value for `vtol_ice_causes_fire`; defaults are centralized below.
    pub vtol_ice_causes_fire: i64,
    /// Configuration value for `glancing_blows`; defaults are centralized below.
    pub glancing_blows: i64,
    /// Configuration value for `inferno_penalty`; defaults are centralized below.
    pub inferno_penalty: i64,
    /// Configuration value for `blzmapmode`; defaults are centralized below.
    pub blzmapmode: i64,
    /// Configuration value for `extended_piloting`; defaults are centralized below.
    pub extended_piloting: i64,
    /// Configuration value for `extended_gunnery`; defaults are centralized below.
    pub extended_gunnery: i64,
    /// Configuration value for `xploss_for_mw`; defaults are centralized below.
    pub xploss_for_mw: i64,
    /// Configuration value for `variable_techtime`; defaults are centralized below.
    pub variable_techtime: i64,
    /// Configuration value for `techtime_mod`; defaults are centralized below.
    pub techtime_mod: i64,
    /// Configuration value for `techtime_multiplier`; defaults are centralized below.
    pub techtime_multiplier: f64,
    /// Configuration value for `statengine_obj`; defaults are centralized below.
    pub statengine_obj: i64,
    /// All-conditions sensor band reach in hexes; weather and darkness do not shorten it.
    pub sensor_range: i64,
    /// Configuration value for `limitedrepairs`; defaults are centralized below.
    pub limitedrepairs: i64,
    /// Configuration value for `stackpole`; defaults are centralized below.
    pub stackpole: i64,
    /// Configuration value for `phys_use_pskill`; defaults are centralized below.
    pub phys_use_pskill: i64,
    /// Configuration value for `erange`; defaults are centralized below.
    pub erange: i64,
    /// Configuration value for `hit_arcs`; defaults are centralized below.
    pub hit_arcs: i64,
    /// Configuration value for `dig_only_fs`; defaults are centralized below.
    pub dig_only_fs: i64,
    /// Configuration value for `digbonus`; defaults are centralized below.
    pub digbonus: i64,
    /// Configuration value for `xp`; defaults are centralized below.
    pub xp: XpConfig,
}
impl Default for BattleTechConfig {
    fn default() -> Self {
        Self {
            explode_reactor: 1,
            explode_time: 120,
            explode_ammo: 1,
            explode_stop: 0,
            parts: 1,
            ic: 1,
            afterlife_dbref: 5,
            vcrit: 2,
            slowdown: 2,
            fasaturn: 1,
            fasaadvvtolcrit: 0,
            fasaadvvhlcrit: 0,
            fasaadvvhlfire: 0,
            divrotordamage: 0,
            moddamagewithrange: 0,
            moddamagewithwoods: 0,
            hotloadaddshalfbthmod: 0,
            nofusionvtolfuel: 0,
            newcharge: 0,
            tl3_charge: 0,
            newterrain: 0,
            xploss: 666,
            critlevel: 100,
            newstagger: 1,
            newstaggertons: 1,
            newstaggertime: 5,
            skidcliff: 0,
            extendedmovemod: 1,
            stacking: 2,
            stackdamage: 100,
            mw_losmap: 1,
            exile_stun_code: 0,
            roll_on_backwalk: 1,
            usedmechstore: 3,
            ooc_comsys: 0,
            idf_requires_spotter: 1,
            tsm_tow_bonus: 1,
            heatcutoff: 1,
            cost_debug: 0,
            allow_cargo_commands: true,
            transported_unit_death: 1,
            mwpickup_action: 1,
            standcareful: 1,
            maxtechtime: 600,
            vtol_ice_causes_fire: 1,
            glancing_blows: 1,
            inferno_penalty: 0,
            blzmapmode: 0,
            extended_piloting: 1,
            extended_gunnery: 1,
            xploss_for_mw: 1,
            variable_techtime: 0,
            techtime_mod: 0,
            techtime_multiplier: 1.0,
            statengine_obj: -1,
            sensor_range: 15,
            limitedrepairs: 0,
            stackpole: 1,
            phys_use_pskill: 1,
            erange: 1,
            hit_arcs: 0,
            dig_only_fs: 0,
            digbonus: 3,
            xp: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed xp config.
pub struct XpConfig {
    /// Configuration value for `bthmod`; defaults are centralized below.
    pub bthmod: i64,
    /// Configuration value for `missilemod`; defaults are centralized below.
    pub missilemod: i64,
    /// Configuration value for `ammomod`; defaults are centralized below.
    pub ammomod: i64,
    /// Configuration value for `defaultweapdam`; defaults are centralized below.
    pub defaultweapdam: i64,
    /// Configuration value for `modifier`; defaults are centralized below.
    pub modifier: i64,
    /// Configuration value for `defaultweapbv`; defaults are centralized below.
    pub defaultweapbv: i64,
    /// Configuration value for `use_pilot_bv_mod`; defaults are centralized below.
    pub use_pilot_bv_mod: i64,
    /// Configuration value for `oldxpsystem`; defaults are centralized below.
    pub oldxpsystem: i64,
    /// Configuration value for `vrtmod`; defaults are centralized below.
    pub vrtmod: i64,
    /// Configuration value for `perunit_xpmod`; defaults are centralized below.
    pub perunit_xpmod: i64,
    /// Configuration value for `noisy_xpgain`; defaults are centralized below.
    pub noisy_xpgain: i64,
    /// Configuration value for `xpgain_cap`; defaults are centralized below.
    pub xpgain_cap: i64,
}
impl Default for XpConfig {
    fn default() -> Self {
        Self {
            bthmod: 0,
            missilemod: 100,
            ammomod: 100,
            defaultweapdam: 5,
            modifier: 100,
            defaultweapbv: 120,
            use_pilot_bv_mod: 1,
            oldxpsystem: 1,
            vrtmod: 0,
            perunit_xpmod: 1,
            noisy_xpgain: 0,
            xpgain_cap: 10,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed mux config.
pub struct MuxConfig {
    /// Configuration value for `badsite_file`; defaults are centralized below.
    pub badsite_file: PathBuf,
    /// Configuration value for `allow_chanlurking`; defaults are centralized below.
    pub allow_chanlurking: i64,
    /// Configuration value for `cache_depth`; defaults are centralized below.
    pub cache_depth: i64,
    /// Configuration value for `cache_trim`; defaults are centralized below.
    pub cache_trim: bool,
    /// Configuration value for `cache_width`; defaults are centralized below.
    pub cache_width: i64,
    /// Configuration value for `check_interval`; defaults are centralized below.
    pub check_interval: i64,
    /// Configuration value for `check_offset`; defaults are centralized below.
    pub check_offset: i64,
    /// Configuration value for `command_quota_increment`; defaults are centralized below.
    pub command_quota_increment: usize,
    /// Configuration value for `command_quota_max`; defaults are centralized below.
    pub command_quota_max: usize,
    /// Configuration value for `conn_timeout`; defaults are centralized below.
    pub conn_timeout: u64,
    /// Configuration value for `connect_dir`; defaults are centralized below.
    pub connect_dir: PathBuf,
    /// Configuration value for `connect_file`; defaults are centralized below.
    pub connect_file: PathBuf,
    /// Configuration value for `connect_reg_file`; defaults are centralized below.
    pub connect_reg_file: PathBuf,
    /// Configuration value for `default_home`; defaults are centralized below.
    pub default_home: i64,
    /// Configuration value for `default_thing_lua_parent`; defaults are centralized below.
    pub default_thing_lua_parent: String,
    /// Configuration value for `default_room_lua_parent`; defaults are centralized below.
    pub default_room_lua_parent: String,
    /// Configuration value for `default_exit_lua_parent`; defaults are centralized below.
    pub default_exit_lua_parent: String,
    /// Configuration value for `default_player_lua_parent`; defaults are centralized below.
    pub default_player_lua_parent: String,
    /// Configuration value for `default_exit_flags`; defaults are centralized below.
    pub default_exit_flags: Vec<Flag>,
    /// Configuration value for `default_player_flags`; defaults are centralized below.
    pub default_player_flags: Vec<Flag>,
    /// Ordered macro set numbers attached to new players, resolved at creation time.
    pub default_player_macros: Vec<usize>,
    /// Configuration value for `default_room_flags`; defaults are centralized below.
    pub default_room_flags: Vec<Flag>,
    /// Configuration value for `default_thing_flags`; defaults are centralized below.
    pub default_thing_flags: Vec<Flag>,
    /// Configuration value for `down_file`; defaults are centralized below.
    pub down_file: PathBuf,
    /// Configuration value for `down_message`; defaults are centralized below.
    pub down_message: String,
    /// Configuration value for `dump_offset`; defaults are centralized below.
    pub dump_offset: i64,
    /// Configuration value for `full_file`; defaults are centralized below.
    pub full_file: PathBuf,
    /// Configuration value for `full_message`; defaults are centralized below.
    pub full_message: String,
    /// Configuration value for `help_directory`; defaults are centralized below.
    pub help_directory: PathBuf,
    /// Configuration value for `idle_interval`; defaults are centralized below.
    pub idle_interval: u64,
    /// Configuration value for `idle_timeout`; defaults are centralized below.
    pub idle_timeout: u64,
    /// Configuration value for `initial_size`; defaults are centralized below.
    pub initial_size: i64,
    /// Configuration value for `max_players`; defaults are centralized below.
    pub max_players: i64,
    /// Configuration value for `notify_recursion_limit`; defaults are centralized below.
    pub notify_recursion_limit: i64,
    /// Configuration value for `output_limit`; defaults are centralized below.
    pub output_limit: i64,
    /// Configuration value for `player_name_spaces`; defaults are centralized below.
    pub player_name_spaces: bool,
    /// Configuration value for `command_queue_limit`; defaults are centralized below.
    pub command_queue_limit: i64,
    /// Configuration value for `player_starting_home`; defaults are centralized below.
    pub player_starting_home: i64,
    /// Configuration value for `player_starting_room`; defaults are centralized below.
    pub player_starting_room: i64,
    /// Configuration value for `public_channel`; defaults are centralized below.
    pub public_channel: String,
    /// Configuration value for `command_queue_active_chunk`; defaults are centralized below.
    pub command_queue_active_chunk: i64,
    /// Configuration value for `command_queue_idle_chunk`; defaults are centralized below.
    pub command_queue_idle_chunk: i64,
    /// Configuration value for `quit_file`; defaults are centralized below.
    pub quit_file: PathBuf,
    /// Configuration value for `retry_limit`; defaults are centralized below.
    pub retry_limit: i64,
    /// Configuration value for `space_compress`; defaults are centralized below.
    pub space_compress: bool,
    /// Configuration value for `stack_limit`; defaults are centralized below.
    pub stack_limit: i64,
    /// Configuration value for `command_quota_interval`; defaults are centralized below.
    pub command_quota_interval: u64,
    /// Configuration value for `unowned_safe`; defaults are centralized below.
    pub unowned_safe: bool,
    /// Configuration value for `player_zone`; defaults are centralized below.
    pub player_zone: i64,
}
impl Default for MuxConfig {
    fn default() -> Self {
        Self {
            badsite_file: PathBuf::from("text/badsite.txt"),
            allow_chanlurking: 0,
            cache_depth: 10,
            cache_trim: false,
            cache_width: 20,
            check_interval: 600,
            check_offset: 300,
            command_quota_increment: 5,
            command_quota_max: 100,
            conn_timeout: 120,
            connect_dir: PathBuf::from(""),
            connect_file: PathBuf::from("text/connect.txt"),
            connect_reg_file: PathBuf::from(""),
            default_home: 0,
            default_thing_lua_parent: "default_thing.lua".into(),
            default_room_lua_parent: "default_room.lua".into(),
            default_exit_lua_parent: "default_exit.lua".into(),
            default_player_lua_parent: "default_player.lua".into(),
            default_exit_flags: vec![Flag::NoCommand],
            default_player_flags: vec![Flag::Ansi, Flag::InCharacter],
            default_player_macros: vec![0],
            default_room_flags: vec![Flag::NoCommand],
            default_thing_flags: vec![],
            down_file: PathBuf::from("text/down.txt"),
            down_message: "".into(),
            dump_offset: 0,
            full_file: PathBuf::from("text/full.txt"),
            full_message: "".into(),
            help_directory: PathBuf::from("help"),
            idle_interval: 60,
            idle_timeout: 3600,
            initial_size: 1000,
            max_players: -1,
            notify_recursion_limit: 20,
            output_limit: 16384,
            player_name_spaces: true,
            command_queue_limit: 100,
            player_starting_home: 4,
            player_starting_room: 4,
            public_channel: "Public".into(),
            command_queue_active_chunk: 10,
            command_queue_idle_chunk: 10,
            quit_file: PathBuf::from("text/quit.txt"),
            retry_limit: 3,
            space_compress: true,
            stack_limit: 50,
            command_quota_interval: 100,
            unowned_safe: false,
            player_zone: 0,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed names config.
pub struct NamesConfig {
    /// Configuration value for `maximum_length`; defaults are centralized below.
    pub maximum_length: usize,
    /// Configuration value for `bad`; defaults are centralized below.
    pub bad: Vec<String>,
    /// Configuration value for `good`; defaults are centralized below.
    pub good: Vec<String>,
}
impl Default for NamesConfig {
    fn default() -> Self {
        Self {
            maximum_length: 30,
            bad: vec![],
            good: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed security config.
pub struct SecurityConfig {
    /// Configuration value for `player_password_length_limit`; defaults are centralized below.
    pub player_password_length_limit: usize,
    /// Configuration value for `password_hash_opslimit`; defaults are centralized below.
    pub password_hash_opslimit: usize,
    /// Configuration value for `password_hash_memlimit`; defaults are centralized below.
    pub password_hash_memlimit: usize,
    /// Configuration value for `login_attempt_burst`; defaults are centralized below.
    pub login_attempt_burst: usize,
    /// Configuration value for `login_attempt_refill`; defaults are centralized below.
    pub login_attempt_refill: u64,
    /// Configuration value for `login_hash_limit`; defaults are centralized below.
    pub login_hash_limit: usize,
    /// Configuration value for `login_address_limit`; defaults are centralized below.
    pub login_address_limit: usize,
    /// Configuration value for `login_address_retention_seconds`; defaults are centralized below.
    pub login_address_retention_seconds: u64,
    /// Configuration value for `login_hash_concurrency`; defaults are centralized below.
    pub login_hash_concurrency: usize,
    /// Configuration value for `login_history_limit`; defaults are centralized below.
    pub login_history_limit: usize,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            player_password_length_limit: 64,
            password_hash_opslimit: 3,
            password_hash_memlimit: 12582912,
            login_attempt_burst: 3,
            login_attempt_refill: 10,
            login_hash_limit: 5,
            login_address_limit: 4096,
            login_address_retention_seconds: 86400,
            login_hash_concurrency: 5,
            login_history_limit: 32,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed logging topics.
pub struct LoggingTopics {
    /// Configuration value for `accounting`; defaults are centralized below.
    pub accounting: bool,
    /// Configuration value for `all_commands`; defaults are centralized below.
    pub all_commands: bool,
    /// Configuration value for `suspect_commands`; defaults are centralized below.
    pub suspect_commands: bool,
    /// Configuration value for `bad_commands`; defaults are centralized below.
    pub bad_commands: bool,
    /// Configuration value for `buffer_alloc`; defaults are centralized below.
    pub buffer_alloc: bool,
    /// Configuration value for `bugs`; defaults are centralized below.
    pub bugs: bool,
    /// Configuration value for `checkpoints`; defaults are centralized below.
    pub checkpoints: bool,
    /// Configuration value for `config_changes`; defaults are centralized below.
    pub config_changes: bool,
    /// Configuration value for `create`; defaults are centralized below.
    pub create: bool,
    /// Configuration value for `logins`; defaults are centralized below.
    pub logins: bool,
    /// Configuration value for `network`; defaults are centralized below.
    pub network: bool,
    /// Configuration value for `problems`; defaults are centralized below.
    pub problems: bool,
    /// Configuration value for `security`; defaults are centralized below.
    pub security: bool,
    /// Configuration value for `shouts`; defaults are centralized below.
    pub shouts: bool,
    /// Configuration value for `startup`; defaults are centralized below.
    pub startup: bool,
    /// Configuration value for `wizard`; defaults are centralized below.
    pub wizard: bool,
}
impl Default for LoggingTopics {
    fn default() -> Self {
        Self {
            accounting: false,
            all_commands: false,
            suspect_commands: false,
            bad_commands: false,
            buffer_alloc: false,
            bugs: true,
            checkpoints: true,
            config_changes: true,
            create: true,
            logins: true,
            network: true,
            problems: true,
            security: true,
            shouts: true,
            startup: true,
            wizard: true,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed logging config.
pub struct LoggingConfig {
    /// Configuration value for `log_options`; defaults are centralized below.
    pub log_options: Vec<LogOption>,
    /// Configuration value for `topics`; defaults are centralized below.
    pub topics: LoggingTopics,
}
impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_options: vec![LogOption::Timestamp, LogOption::Location],
            topics: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
/// Typed access config.
pub struct AccessConfig {
    /// Configuration value for `commands`; defaults are centralized below.
    pub commands: BTreeMap<String, Permissions>,
    /// Configuration value for `lists`; defaults are centralized below.
    pub lists: BTreeMap<String, Permissions>,
    /// Configuration value for `config`; defaults are centralized below.
    pub config: BTreeMap<String, Permissions>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
/// Typed aliases config.
pub struct AliasesConfig {
    /// Configuration value for `commands`; defaults are centralized below.
    pub commands: BTreeMap<String, String>,
    /// Configuration value for `flags`; defaults are centralized below.
    pub flags: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
/// Typed osc8 config.
pub struct Osc8Config {
    /// Configuration value for `presets`; defaults are centralized below.
    pub presets: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
/// Typed sites config.
pub struct SitesConfig {
    /// Configuration value for `forbid`; defaults are centralized below.
    pub forbid: Vec<SiteRule>,
    /// Configuration value for `suspect`; defaults are centralized below.
    pub suspect: Vec<SiteRule>,
    /// Configuration value for `trust`; defaults are centralized below.
    pub trust: Vec<SiteRule>,
    /// Configuration value for `permit`; defaults are centralized below.
    pub permit: Vec<SiteRule>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// Typed runtime config.
pub struct RuntimeConfig {
    /// Configuration value for `max_connections`; defaults are centralized below.
    pub max_connections: usize,
    /// Configuration value for `event_queue_capacity`; defaults are centralized below.
    pub event_queue_capacity: usize,
    /// Configuration value for `session_output_queue_capacity`; defaults are centralized below.
    pub session_output_queue_capacity: usize,
    /// Configuration value for `input_line_limit`; defaults are centralized below.
    pub input_line_limit: usize,
    /// Configuration value for `telnet_subnegotiation_limit`; defaults are centralized below.
    pub telnet_subnegotiation_limit: usize,
    /// Configuration value for `output_message_limit`; defaults are centralized below.
    pub output_message_limit: usize,
    /// Configuration value for `write_timeout_ms`; defaults are centralized below.
    pub write_timeout_ms: u64,
    /// Configuration value for `shutdown_timeout_ms`; defaults are centralized below.
    pub shutdown_timeout_ms: u64,
    /// Configuration value for `maintenance_interval_ms`; defaults are centralized below.
    pub maintenance_interval_ms: u64,
}
impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_connections: 1024,
            event_queue_capacity: 256,
            session_output_queue_capacity: 128,
            input_line_limit: 8192,
            telnet_subnegotiation_limit: 8192,
            output_message_limit: 65536,
            write_timeout_ms: 5000,
            shutdown_timeout_ms: 5000,
            maintenance_interval_ms: 1000,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
/// Typed settings.
pub struct Settings {
    /// Configuration value for `database`; defaults are centralized below.
    pub database: DatabaseConfig,
    /// Configuration value for `lua`; defaults are centralized below.
    pub lua: LuaConfig,
    /// Configuration value for `server`; defaults are centralized below.
    pub server: ServerConfig,
    /// Configuration value for `battletech`; defaults are centralized below.
    pub battletech: BattleTechConfig,
    /// Configuration value for `mux`; defaults are centralized below.
    pub mux: MuxConfig,
    /// Configuration value for `security`; defaults are centralized below.
    pub security: SecurityConfig,
    /// Configuration value for `names`; defaults are centralized below.
    pub names: NamesConfig,
    /// Configuration value for `logging`; defaults are centralized below.
    pub logging: LoggingConfig,
    /// Configuration value for `access`; defaults are centralized below.
    pub access: AccessConfig,
    /// Configuration value for `aliases`; defaults are centralized below.
    pub aliases: AliasesConfig,
    /// Configuration value for `osc8`; defaults are centralized below.
    pub osc8: Osc8Config,
    /// Configuration value for `sites`; defaults are centralized below.
    pub sites: SitesConfig,
    /// Configuration value for `runtime`; defaults are centralized below.
    pub runtime: RuntimeConfig,
    /// Configuration value for `colors`; defaults are centralized below.
    pub colors: BTreeMap<String, Rgb>,
}
