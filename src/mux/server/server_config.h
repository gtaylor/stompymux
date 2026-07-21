/* server_config.h - Runtime configuration, access, and logging constants. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/commands/command_queue.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_registries.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/red_black_tree.h"
#include "mux/world/player_cache.h"
#include <netinet/in.h>

typedef struct Descriptor Descriptor;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct HelpIndex HelpIndex;
typedef struct LoginThrottle LoginThrottle;
typedef struct PlayerCache PlayerCache;
typedef struct FileCache FileCache;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct CommandQueue CommandQueue;
typedef struct ChannelRegistry ChannelRegistry;
typedef struct LogCache LogCache;
typedef struct LuaRuntime LuaRuntime;
typedef struct MacroRegistry MacroRegistry;
typedef struct PersistenceContext PersistenceContext;

/* ServerConfiguration:	runtime configurable parameters */

typedef unsigned char Uchar;

typedef struct DatabaseConfiguration DatabaseConfiguration;
struct DatabaseConfiguration {
  char gamedb[128];  /* SQLite game database */
  char mech_db[128]; /* Mecha templates */
  char map_db[128];  /* Map templates */
  int dump_interval; /* Interval between checkpoint dumps in seconds */
};

typedef struct LuaConfiguration LuaConfiguration;
struct LuaConfiguration {
  char directory[128];   /* Lua module root, relative to game directory */
  int instruction_limit; /* Lua VM instructions per callback */
  int memory_limit;      /* Lua VM memory cap in bytes */
};

typedef struct ServerConfiguration ServerConfiguration;
struct ServerConfiguration {
  bool is_initializing;           /* Are we reading the startup config? */
  int cache_trim;                 /* Should cache be shrunk to original size */
  int cache_steal_dirty;          /* Should cache code write dirty attrs */
  int cache_depth;                /* Number of entries in each cache cell */
  int cache_width;                /* Number of cache cells */
  int cache_names;                /* Should object names be cached separately */
  DatabaseConfiguration database; /* Database configuration */
  LuaConfiguration lua;           /* Lua runtime configuration */
  char config_file[128];          /* name of configuration file */
  int have_specials;              /* Should the special hcode be active? */
  int have_comsys;                /* Should the comsystem be active? */
  int have_macros;                /* Should the macro system be active? */
  int have_zones;                 /* Should zones be active? */
  int port;                       /* user port */
  int conc_port;                  /* concentrator port */
  int init_size;                  /* initial db size */
  char conn_file[32];             /* display on connect */
  char conn_dir[32];              /* display on connect */
  char quit_file[32];             /* display on quit */
  char down_file[32];             /* display this file if no logins */
  char full_file[32];             /* display when max users exceeded */
  char site_file[32];             /* display if conn from bad site */
  char help_dir[128];        /* Help article root, relative to game directory */
  char down_msg[4096];       /* Message displayed when logins are disabled */
  char full_msg[4096];       /* Message displayed when the game is full */
  char dump_msg[128];        /* Message displayed when @dump-ing */
  char postdump_msg[128];    /* Message displayed after @dump-ing */
  char fixed_home_msg[128];  /* Message displayed when going home and FIXED */
  char fixed_tel_msg[128];   /* Message displayed when teleporting and FIXED */
  char public_channel[32];   /* Name of public channel */
  int btech_explode_reactor; /* Allow or disallow explode reactor */
  int btech_explode_ammo;    /* Allow or disallow explode ammo */
  int btech_explode_stop;    /* Allow or disallow explode stop */
  int btech_explode_time;    /* Number of tics self-destruction takes */
  int btech_ic;       /* Allow or disallow MechWarrior embark/disembark */
  int btech_parts;    /* If 1, enable part brands. */
  int btech_vcrit;    /* If below 2, vehicles don't crit at all. */
  int btech_slowdown; /* If 2, new slowdown code based on desired/current
                         heading. 1 is a flat reduction. 0 is no slowdown. */
  int btech_fasaturn;
  int btech_dynspeed;  /* Factor in section loss and gravity for mech speed. */
  int btech_stackpole; /* Mechs mushroom as a result of triple engine crits */
  int btech_erange;    /* 1= Enable extended, extended weapon ranges. */
  int btech_hit_arcs;  /* hit arc rules (see FindAreaHitGroup()) */
  int btech_phys_use_pskill;    /* Use piloting skills for physical attacks */
  int allow_chanlurking;        /* 'last' and 'who' on 'off' channels ? */
  int btech_newterrain;         /* fasa terrain restrictions for wheeled */
  int btech_fasacrit;           /* fasa critsystem */
  int btech_fasaadvvtolcrit;    /* L3 FASA VTOL crits and hitlocs */
  int btech_fasaadvvhlcrit;     /* L3 FASA ground vehicle crits and hitlocs */
  int btech_fasaadvvhlfire;     /* L3 FASA ground vehicle fire rules */
  int btech_divrotordamage;     /* amount to divide damage to vtol rotors by.
                                   Instead of just 1 pt per hit, we'll divide the
                                   damage by something */
  int btech_moddamagewithrange; /* For energy weapons: +1 damage at <=1 hex, -1
                                   damage at long range */
  int btech_moddamagewithwoods; /* Occupied woods do not add to BTH but lessen
                                   damage. -2 for light, -4 for heavy, chance to
                                   clear on each shot. Intervening woods don't
                                   change. */
  int btech_hotloadaddshalfbthmod; /* Mod the BTH for hotloaded LRMs. The BTH
                                      mod is half what it is if not hotloaded */
  int btech_nofusionvtolfuel;      /* Fusion engine'd VTOLs don't use fuel */
  int btech_vhltacthreshold;  /* threshold for vehicle TACs. If it's <= 0, we
                                 ignore any armor level and do normal tacs, else
                                 we only TAC if the armor percent is <= to the
                                 value. */
  int btech_mechtacthreshold; /* threshold for Mech TACs. If it's <= 0, we
                                 ignore any armor level and do normal tacs, else
                                 we only TAC if the armor percent is <= to the
                                 value. */
  int btech_newcharge;        /* 1= use distance counter for charge damage. */
  int btech_tl3_charge; /* 1= New TL3 damage formula. Takes direction and speed
                           into account. */
  int btech_tankfriendly; /* Some tank friendly changes if fasacrit is too harsh
                           */
  int btech_skidcliff; /* skidroll to check for cliffs and falldamage for mechs
                        */
  int btech_xp_bthmod; /* Use bth modifier in new xp code */
  int btech_xp_missilemod;    /* Modifier for missile weapons */
  int btech_xp_ammomod;       /* modifier for ammo weapons (not missiles ) */
  int btech_defaultweapdam;   /* modifier to default weapon BV */
  int btech_xp_usePilotBVMod; /* use the pilot's skills to modify the BV of the
                                 unit */
  int btech_xp_modifier;      /* modifier to increase or decrease xp-gain */
  int btech_defaultweapbv;    /* Weapons with BVs higher than this give less xp,
                                 lower give more */
  int btech_oldxpsystem;      /* Uses old xp system if 1 */
  int btech_xp_vrtmod;        /* Modifier for VRT weapons used if !0 */
  int btech_limitedrepairs; /* If on then armor fixes and reloads in stalls only
                             */
  int btech_digbonus; /* If shot would land on the FS hitgroup of a tank at >=
                         your elevation, add this number to BTH. */
  int btech_dig_only_fs; /* Make the bonus apply to only shots that would hit
                            the tank's FS hitgroup. */
  int btech_xploss; /* Percentage of XP to lose on dying. 1000 = none, 0 = all?
                     */
  int btech_critlevel;       /* percentage of armor left before TAC occurs */
  int btech_tankshield;      /* ???: Not really sure. */
  int btech_newstagger;      /* For the new round based stagger */
  int btech_newstaggertons;  /* For the new stagger tonnage mod (0 turns off
                                stagger based on tonnage) */
  int btech_newstaggertime;  /* Time between stagger checks */
  int btech_extendedmovemod; /* Whether to use MaxTech's extended target
                                movement modifiers */
  int btech_stacking; /* Whether to check for stacking, and how to penalize */
  int btech_stackdamage;         /* Damage modifier for btech_stacking=2 */
  int btech_mw_losmap;           /* Whether MechWarriors always get a losmap */
  int btech_seismic_see_stopped; /* Whether you see stopped mechs on seismic */
  int btech_exile_stun_code;  /* Should we use the Exile Head Hit Stun code. Set
                                 to 2 to not actually stun, but reassign headhits
                               */
  int btech_roll_on_backwalk; /* wheter a piloting roll should be made to walk
                                 backwards over elevation changes */
  int btech_usedmechstore;    /* DBref for the dead mechs to spool into upon IC
                                 death */
  int btech_ooc_comsys; /* Enable bypassing of CA_NO_IC command checks for IC
                           location blocking */
  int btech_idf_requires_spotter; /* Requires spotter for IDF firing */
  int btech_vtol_ice_causes_fire; /* VTOL ICE engines cause fire on
                                     crash/explosion */
  int btech_glancing_blows;  /* 0=Don't, 1=maxtech (BTH) , 2= Exile (BTH-1) */
  int btech_inferno_penalty; /* FASA Inferno Ammo penalty (+30 heat, ammo
                                explode) */
  int btech_perunit_xpmod;   /* Allow per unit xp modifications */
  int btech_tsm_tow_bonus;   /* Give bonus to TSM units when towing, similiar to
                                salvage tech (1=Yes, 0=No) */
  int btech_tsm_sprint_bonus; /* 0= sprint and tsm don't stack 1= stack sprint
                                 and tsm */
  int btech_heatcutoff;       /* 0= The 'heatcutoff' command is disabled. */
  int btech_sprint_bth;       /* BTH to give for attacks against sprinting units
                                 (default is -4)*/
  int btech_cost_debug; /* 1= Send info from btfasabasecost to MechDebugInfo */
  int btech_noisy_xpgain;           /* 1 = Send XP Gain info to MechXP */
  int btech_xpgain_cap;             /* Cap for Weapons XP Gain */
  int btech_transported_unit_death; /* 1=Destroy units in a transport
                                       automatically. (Via AMECHDEST) 0=Don't.
                                     */
  int btech_mwpickup_action;        /* 0 = quiet teleport,
                                       1 = TELE_LOUD (triggers AENTER) */
  int btech_standcareful; /* 0 = Don't allow (FASA rules), 1 Allow (-2 BTH to
                             stand, double time if successful */
  int btech_maxtechtime;  /* Max Tech Time allowed, in minutes */
  int btech_blzmapmode;   /* 0 = Regular tacmap for non-aeros <blzs with terrain
                             blockouts> (Default and usual), 1 = Just BLZs <no
                             terrain blockers> */
  int btech_extended_piloting; /* 0 = No (Drive, Piloting-Bmech, etc) 1 = Yes
                                  (Piloting-Tracked, Piloting-Biped, etc.) */
  int btech_extended_gunnery;  /* 0 = No (Gunnery-Bmech, Gunnery-Aero, etc) 1 =
                                  Yes (Gunnery-Laser, Gunnery-Missile, etc.) */
  int btech_xploss_for_mw;     /* 0 = No (Mechwarrior death !=XP Loss.) 1 = Yes
                                  (Default. MW death = XPLOSS if IC) */
  int btech_variable_techtime; /* 0 = No (Normal Techtime) 1 = Yes (Techtime is
                                  modded by btech_techtime_mod per roll above
                                  tofix) */
  int btech_techtime_mod;   /* BTH: 6, Roll 7 would be 1 * techtime_mod , BTH 6,
                               Roll 8, would 2 be 2 *, etc. Lowers techtime based
                               on 'good' rolls */
  int btech_statengine_obj; /* Object to send stats on hits/crits to. Defaults
                               to -1 (off) */
#ifdef BT_FREETECHTIME
  int btech_freetechtime; /* Near instant repair times */
#endif
#ifdef BT_COMPLEXREPAIRS
  int btech_complexrepair;
#endif
  int afterlife_dbref;
  int indent_desc;         /* Newlines before and after descs? */
  int name_spaces;         /* allow player names to have spaces */
  int show_unfindable_who; /* should players set UNFINDABLE appear on who? */
  int fork_dump;           /* perform dump in a forked process */
  int paranoid_alloc;      /* Rigorous buffer integrity checks */
  int max_players;         /* Max # of connected players */
  int check_interval;      /* interval between db check/cleans in secs */
  int dump_offset;         /* when to take first checkpoint dump */
  int check_offset;        /* when to perform first check and clean */
  int idle_timeout;        /* Boot off players idle this long in secs */
  int conn_timeout;        /* Allow this long to connect before booting */
  int idle_interval;       /* when to check for idle users */
  int retry_limit;         /* close conn after this many bad logins */
  int player_password_length_limit; /* Maximum length of a player password */
  int password_hash_opslimit;       /* Argon2id CPU cost */
  int password_hash_memlimit;       /* Argon2id memory cost in bytes */
  int login_attempt_burst;          /* Per-source login attempts before delay */
  int login_attempt_refill;         /* Seconds to refill one login attempt */
  int login_hash_limit;             /* Password checks permitted per second */
  int output_limit;                 /* Max # chars queued for output */
  int use_http;                     /* Should we allow http access? */
  int queuemax;       /* max commands a player may have in queue */
  int queue_chunk;    /* # cmds to run from queue when idle */
  int active_q_chunk; /* # cmds to run from queue when active */
  int ex_flags;       /* TRUE = show flags on examine */
  int robot_speak;    /* TRUE = allow robots to speak */
  int dark_sleepers;  /* Are sleeping players 'dark'? */
  int idle_wiz_dark;  /* Do idling wizards get set dark? */
  int switch_df_all;  /* Should @switch match all by default? */
  int fascist_tport;  /* Source of teleport must be controlled */
  int trace_topdown;  /* Is TRACE output top-down or bottom-up? */
  int trace_limit;    /* Max lines of trace output if top-down */
  int stack_limit;    /* How big can stacks get? */
  int safe_unowned;   /* Are objects not owned by you safe? */
  int space_compress; /* Convert multiple spaces into one space */
  int start_room;     /* initial location and home for players */
  int start_home;     /* initial HOME for players */
  int default_home;   /* HOME when home is inaccessable */
  char default_thing_lua_parent[128];  /* Lua parent for new things */
  char default_room_lua_parent[128];   /* Lua parent for new rooms */
  char default_exit_lua_parent[128];   /* Lua parent for new exits */
  char default_player_lua_parent[128]; /* Lua parent for new players */
  FLAGSET default_player_flags;        /* Flags players start with */
  FLAGSET default_room_flags;          /* Flags rooms start with */
  FLAGSET default_exit_flags;          /* Flags exits start with */
  FLAGSET default_thing_flags;         /* Flags things start with */
  FLAGSET robot_flags;                 /* Flags robots start with */
  char mud_name[32];                   /* Name of the mud */
  int timeslice;      /* How often do we bump people's cmd quotas? */
  int cmd_quota_max;  /* Max commands at one time */
  int cmd_quota_incr; /* Bump #cmds allowed by this each timeslice */

  bool is_login_enabled;         /* Allow nonwizard logins */
  bool is_interpreter_enabled;   /* Allow object triggering */
  bool is_checkpointing_enabled; /* Perform automatic checkpoints */
  bool is_db_check_enabled;      /* Periodically check and clean the DB */
  bool is_idle_check_enabled;    /* Periodically check for idle users */
  bool is_dequeue_enabled;       /* Remove entries from the command queue */

  int log_options;   /* What gets logged */
  int log_info;      /* Info that goes into log entries */
  int func_nest_lim; /* Max nesting of functions */
  int func_invk_lim; /* Max funcs invoked by a command */
  int ntfy_nest_lim; /* Max nesting of notifys */
  int zone_nest_lim; /* Max nesting of zones */
  int player_zone;
};

typedef struct SiteData SiteData;
struct SiteData {
  struct SiteData *next;  /* Next site in chain */
  struct in_addr address; /* Host or network address */
  struct in_addr mask;    /* Mask to apply before comparing */
  int flag;               /* Value to return on match */
};

typedef struct badname_struc BADNAME;
struct badname_struc {
  char *name;
  struct badname_struc *next;
};

/* Global flags */

/* Host information codes */

constexpr int H_FORBIDDEN = 0x0002; /* Reject all connects */
constexpr int H_SUSPECT = 0x0004;   /* Notify wizards of connects/disconnects */

/* Logging options */

constexpr int LOG_ALLCOMMANDS = 0x00000001; /* Log all commands */
constexpr int LOG_ACCOUNTING = 0x00000002; /* Write accounting info on logout */
constexpr int LOG_BADCOMMANDS = 0x00000004; /* Log bad commands */
constexpr int LOG_BUGS = 0x00000008;        /* Log program bugs found */
constexpr int LOG_DBSAVES = 0x00000010;     /* Log database dumps */
constexpr int LOG_CONFIGMODS = 0x00000020;  /* Log changes to configuration */
constexpr int LOG_PCREATES = 0x00000040;    /* Log character creations */
/* 0x00000080 is reserved for the removed killing log category. */
constexpr int LOG_LOGIN = 0x00000100;    /* Log logins and logouts */
constexpr int LOG_NET = 0x00000200;      /* Log net connects and disconnects */
constexpr int LOG_SECURITY = 0x00000400; /* Log security-related events */
constexpr int LOG_SHOUTS = 0x00000800;   /* Log shouts */
constexpr int LOG_STARTUP = 0x00001000;  /* Log nonfatal errors in startup */
constexpr int LOG_WIZARD = 0x00002000;   /* Log dangerous things */
constexpr int LOG_ALLOCATE = 0x00004000; /* Log alloc/free from buffer pools */
constexpr int LOG_PROBLEMS = 0x00008000; /* Log runtime problems */
constexpr int LOG_SUSPECTCMDS =
    0x00010000; /* Log commands by people set SUSPECT */
// Stored as int (not unsigned) so it combines cleanly with the other LOG_*
// bitmask constants and configuration log options without conversions;
// C23 guarantees the top-bit pattern converts to INT_MIN deterministically.
constexpr int LOG_ALWAYS = (int)0x80000000U; /* Always log it */

constexpr int LOGOPT_FLAGS = 0x01;     /* Report flags on object */
constexpr int LOGOPT_LOC = 0x02;       /* Report loc of obj when requested */
constexpr int LOGOPT_OWNER = 0x04;     /* Report owner of obj if not obj */
constexpr int LOGOPT_TIMESTAMP = 0x08; /* Timestamp log entries */

constexpr int HIDDEN_IDLESECS = 600; /* Show people idle for less as 0s idle */
