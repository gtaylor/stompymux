#include "btech/context.h"
#include "mux/server/platform.h"

#pragma once

/*
 * Define if you want BV calculation functions. This include btgetbv()
 * and btgetbv_ref(), but will also perform damage per time based updates
 * on the mech live to support more balance XP ratio's.
 */
#define BT_CALCULATE_BV

/* Define if you want Variable Recycle Times for weapons */
#define BT_USE_VRT

/* Define if you want part-specific weights for cargo */
#define BT_PART_WEIGHTS

/* Define for heatsink on/off msgs when using heatcutoff */
/* #define HEATCUTOFF_DEBUG */

/* Define if you want weight-class based 'status' pictures */
#define WEIGHTVARIABLE_STATUS

/*
 * Define if you want the radio OBSERVATORIC units to be more
 * Observation-lounge like: hears everything, and reports more.
 * */
#define OBSERVATORIC_OL_RADIO

/*
 * This is the maximum amount of parts addable via btaddstores() or the
 * addstuff command. If the limit is hit, set the number of commods to add
 * equal to this define.
 */
constexpr int ADDSTORES_MAX = 50000;

constexpr int RS_MECH_IDLE = 86400;
constexpr int SIM_MECH_IDLE = 3600;

/* Where the dogfighting becomes 'fun' */
constexpr int ATMO_Z = 100;

/* Orbit elevation */
constexpr int ORBIT_Z = 300;

/* At max 5x */
constexpr int ACCEL_MOD = 5;

/* How many secs it takes to apply one maxthrust
   (mod'ed by location _and_ type of craft) */
constexpr int AERO_SECS_THRUST = 30;

constexpr int PIL_XP_EVERY_N_STEPS = 10;

#define MINE_NEXT_MODIFIER (2 / 3)
constexpr int MINE_MIN = 5;
constexpr int MINE_TABLE = 2; /* 0 = General, 2 = KICK */

/* Have weird jump code? (undef = basic MUSE one) */
#define ODDJUMP

/* Whether we want 'BT' partial or not */
#define BT_PARTIAL

/* unload / load, addstuff / removestuff multiple kinds of items at
   once */
#define ECON_ALLOW_MULTIPLE_LOAD_UNLOAD

/* Whether we acknowledge Munchkins exist or not */
#define CLAN_SUPPORT

/* Whether we support C3 or not */
#define C3_SUPPORT

/* Show BTHs on Debug */
#define BTH_DEBUG

/* Show some XP calculation messages on Debug */
#define XP_DEBUG

/* Shows ton of unneccessary debug messages */
#undef TEMPLATE_DEBUG

/* Show jump coords on Debug */
#undef JUMPDEBUG

/* Show sensor BTHs on Debug */
#undef SENSOR_BTH_DEBUG

/* Don't see see/dontsee msgs */
#undef SENSOR_DEBUG

/* Shows errors whenever need be */
#define TEMPLATE_VERBOSE_ERRORS

/* Show loading / saving of map stuff specifically */
#undef VERBOSE_MAP_STUFF

/* Define if buildings should regenerate CF */
#define BUILDINGS_REPAIR_THEMSELVES
#define BUILDINGS_REBUILD_FROM_DESTRUCTION

constexpr int BUILDING_REPAIR_DELAY = 120; /* 1 pt / 1 min */

/* Howlong to wait before rebuilding cf0'd buildings. */
constexpr int BUILDING_DREBUILD_DELAY = 7200; /* 2 hours */

/* Define if ECM status (ECM active, ECCM active, ECM-disturbed and
 * ECM-protected) should show on contacts (as 'E', 'P', 'e' and 'p') */
#define ECM_ON_CONTACTS

constexpr int LATERAL_TICK = 6;
constexpr int HEAT_TICK = 2;
constexpr int JUMP_TICK = 1;
constexpr int MOVE_TICK = 1; /* How oft da mecha move ;-) */
constexpr double MOVE_MOD = .5;
constexpr int WEAPON_TICK = 2;

constexpr int ARTY_SPEED = 5; /* Artillery round flies 5 hexes / second */
constexpr int ARTILLERY_MAPSHEET_SIZE = 20; /* Size of single arty mapsheet */
constexpr int ARTILLERY_MINIMUM_FLIGHT =
    10; /* How long's the minimum flight time */

#define DROP_TO_STAND_RECYCLE (MOVE_TICK * 12)
#define JUMP_TO_HIT_RECYCLE                                                    \
  (JUMP_TICK * 12 / (MechType(mech) == CLASS_BSUIT ? 4 : 1))

constexpr int INITIAL_PLOS_TICK = 1; /* How many secs after startup */
constexpr int LOS_TICK = 1;          /* Update LOS tables */
constexpr int HIDE_TICK = 10;
constexpr int PLOS_TICK = 1;     /* How many seconds interval between checks */
constexpr int SCHANGE_TICK = 10; /* Sensor change */
constexpr int SPOT_TICK = 10; /* How oft is the range for spotting checked? */

#define PHYSICAL_RECYCLE_TIME (30 * WEAPON_TICK)
constexpr int STARTUP_TIME = 30;
constexpr int UNCONSCIOUS_TIME =
    30; /* ORIGINAL authors thought it was UNCONCIOUS */
constexpr int WEAPON_RECYCLE_TIME = 30; /* weapon_tick's */
constexpr int FALL_TICK = 3;            /* How oft do we call fall event? */
constexpr int FALL_ACCEL = 1; /* How much do we accelerate each event? */
constexpr int OOD_SPEED = 2;  /* 2 Z / tic ; 150 sec for landing */
constexpr int OOD_TICK = 1;
constexpr int DUMP_TICK =
    30; /* How long does it take to eject 1 ton of ammo? */
constexpr int DUMP_GRAD_TICK = 1; /* This oft we _maybe_ dump stuff */
#define DUMP_SPEED (DUMP_TICK / DUMP_GRAD_TICK)
constexpr int MASC_TICK = 60;    /* Time for each MASC regen / fail */
constexpr int SCHARGE_TICK = 60; /* Time for each Supercharger regen / fail */
#define RANDOM_TICK                                                            \
  6 /* How many seconds do we want to use same rnd# for                        \
       BTHs etc */
#define DS_SPAM_TIME                                                           \
  10 /* At max, 1 mapemit every 10 secs concerning a                           \
        single DS */

#define MAX_BOOM_TIME                                                          \
  30                        /* Max time between first and last CT int hit for  \
                               fusion explosion */
constexpr int BOOM_BTH = 9; /* Roll below this or 'boom' */
constexpr int MAX_C3_SLAVES = 3;

#define CHARGE_TIMER_LIMIT                                                     \
  60 /* How long should we let them 'charge' for (in seconds) */
#define CHARGE_DIST_TRIGGER                                                    \
  0.6 /* At what range should we trigger the charge (hexes) */

/* Skills used if pilot's not valid and no default mech skills */
constexpr int DEFAULT_GUNNERY = 6;
constexpr int DEFAULT_PILOTING = 6;
constexpr int DEFAULT_SPOTTING = 8;
constexpr int DEFAULT_ARTILLERY = 8;
constexpr int DEFAULT_COMM = 6;

/* Default ranges and stuff */
constexpr int DEFAULT_TACRANGE = 20;
constexpr int DEFAULT_LRSRANGE = 40;
constexpr int DEFAULT_RADIORANGE = 80;
constexpr int DEFAULT_SCANRANGE = 20;
constexpr int DEFAULT_HEATSINKS = 10;

/* IS guys suck */
constexpr int DEFAULT_COMPUTER = 2;
constexpr int DEFAULT_RADIO = 3;
constexpr int DEFAULT_PART_LEVEL = 3;

/* Clans get better stuff */
constexpr int DEFAULT_CLCOMPUTER = 5;
constexpr int DEFAULT_CLRADIO = 5;
constexpr int DEFAULT_CLPART_LEVEL = 5;

/* Display Types */
constexpr int LRS_DISPLAY_WIDTH = 70;
constexpr int LRS_DISPLAY_WIDTH2 = 35;
constexpr int LRS_DISPLAY_HEIGHT = 11;
constexpr int LRS_DISPLAY_HEIGHT2 = 5;

/* Sensor Stuff */
#define ADVANCED_LOS
#define LOCK_TICK 8

constexpr int ECM_RANGE = 6;

/* What kind of evil magic DFM's affect */
#undef DFM_AFFECT_BTH

constexpr int LITE_RANGE = 30;

typedef unsigned char Byte;

/* Exile Stun Code Timer */
constexpr int MECHSTUN_TICK = 10;
