/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#pragma once

#include "section_types.h"
/* status element... */

constexpr int LANDED = 0x00000001;        /* (a) For VTOL use only */
constexpr int TORSO_RIGHT = 0x00000002;   /* (b) Torso heading -= 60 degrees */
constexpr int TORSO_LEFT = 0x00000004;    /* (c) Torso heading += 60 degrees */
constexpr int STARTED = 0x00000008;       /* (d) Mech is warmed up */
constexpr int PARTIAL_COVER = 0x00000010; /* (e) */
constexpr int DESTROYED = 0x00000020;     /* (f) */
constexpr int JUMPING = 0x00000040;       /* (g) Handled in UPDATE */
constexpr int FALLEN = 0x00000080;        /* (h) */
constexpr int DFA_ATTACK = 0x00000100;    /* (i) */
#define PERFORMING_ACTION                                                      \
  0x00000200 /* (j) Set if the unit is performing some sort of action.         \
                Controlled by SCode */
constexpr int FLIPPED_ARMS = 0x00000400; /* (k) */
#define AMS_ENABLED                                                            \
  0x00000800 /* (l) only settable if mech has ANTI-MISSILE_TECH */
#define EXPLODE_SAFE                                                           \
  0x00001000 /* (m) Used to prevent a unit from doing EXPLODE AMMO */
constexpr int UNCONSCIOUS = 0x00002000;   /* (n) Pilot is unconscious */
constexpr int TOWED = 0x00004000;         /* (o) Someone's towing us */
constexpr int LOCK_TARGET = 0x00008000;   /* (p) We mean business */
constexpr int LOCK_BUILDING = 0x00010000; /* (q) Hit building */
#define LOCK_HEX                                                               \
  0x00020000 /* (r) Hit hex (clear / ignite, d'pend on weapon)                 \
              */
constexpr int LOCK_HEX_IGN = 0x00040000; /* (s) */
constexpr int LOCK_HEX_CLR = 0x00080000; /* (t) */
constexpr int MASC_ENABLED = 0x00100000; /* (u) Using MASC */
#define BLINDED                                                                \
  0x00200000 /* (v) Pilot has been blinded momentarily by something */
constexpr int COMBAT_SAFE = 0x00400000; /* (w) Can't be hurt */
#define AUTOCON_WHEN_SHUTDOWN                                                  \
  0x00800000                      /* (x) Autocon sees it even when shutdown */
constexpr int FIRED = 0x01000000; /* (y) Fired at something */
constexpr int SCHARGE_ENABLED = 0x02000000;  /* (z) */
constexpr int HULLDOWN = 0x04000000;         /* (A) */
constexpr int UNDERSPECIAL = 0x08000000;     /* (B) */
constexpr int UNDERGRAVITY = 0x10000000;     /* (C) */
constexpr int UNDERTEMPERATURE = 0x20000000; /* (D) */
constexpr int UNDERVACUUM = 0x40000000;      /* (E) */
/* UNUSED                     0x80000000     (F) */

#define CONDITIONS                                                             \
  (UNDERSPECIAL | UNDERGRAVITY | UNDERTEMPERATURE | UNDERVACUUM)
#define LOCK_MODES                                                             \
  (LOCK_TARGET | LOCK_BUILDING | LOCK_HEX | LOCK_HEX_IGN | LOCK_HEX_CLR)

/* status2 element */

/* Specials status element */
constexpr int ECM_ENABLED = 0x00000001;     /* (a) Unit ECM is enabled */
constexpr int ECCM_ENABLED = 0x00000002;    /* (b) Unit ECCM is enabled */
constexpr int ECM_DISTURBANCE = 0x00000004; /* (c) Unit affected by ECM */
constexpr int ECM_PROTECTED = 0x00000008;   /* (d) Unit protected by ECM */
#define ECM_COUNTERED                                                          \
  0x00000010                             /* (e) ECM countered by ECCM.         \
                                            This only happens if an enemy ECCM \
                                            is within range. */
constexpr int SLITE_ON = 0x00000020;     /* (f) Unit SLITE is enabled */
constexpr int STH_ARMOR_ON = 0x00000040; /* (g) Unit has steath armor enabled */
constexpr int NULLSIGSYS_ON = 0x00000080; /* (h) Unit has nullsig enabled */
constexpr int ANGEL_ECM_ENABLED =
    0x00000100; /* (i) Unit Angel ECM is enabled */
constexpr int ANGEL_ECCM_ENABLED =
    0x00000200; /* (j) Unit Angel ECCM is enabled */
constexpr int ANGEL_ECM_PROTECTED =
    0x00000400; /* (k) Unit protected by Angel ECM */
constexpr int ANGEL_ECM_DISTURBED =
    0x00000800; /* (l) Unit affected by Angel ECM */
constexpr int PER_ECM_ENABLED =
    0x00001000; /* (m) Unit Personal ECM is enabled */
constexpr int PER_ECCM_ENABLED =
    0x00002000; /* (n) Unit Personal ECCM is enabled */
#define AUTOTURN_TURRET                                                        \
  0x00004000 /* (o) Unit Auto-Turret enabled to locked target */
/* UNUSED                     0x00008000     (p) */
constexpr int SPRINTING = 0x00010000; /* (q) Unit is Sprinting */
constexpr int EVADING = 0x00020000;   /* (r) Unit is Evading */
constexpr int DODGING = 0x00040000;   /* (s) Unit is Dodging */
#define ATTACKEMIT_MECH                                                        \
  0x00080000 /* (t) Units attacks sent to MechAttackEmits channel */
constexpr int UNIT_MOUNTED =
    0x00100000; /* (u) Unit has been mounted by a suit */
constexpr int UNIT_MOUNTING =
    0x00200000;                          /* (v) Unit is mounting another unit */
constexpr int FORTIFIED = 0x00400000;    /* (w) */
constexpr int WEAPONS_HOLD = 0x00800000; /* (x) Unit is unable to fire */
constexpr int NO_GUN_XP =
    0x01000000; /* (y) Don't give gun xp if we fire at this */
/* UNUSED                     0x02000000     (z) */
/* UNUSED                     0x04000000     (A) */
/* UNUSED                     0x08000000     (B) */
/* UNUSED                     0x10000000     (C) */
/* UNUSED                     0x20000000     (D) */
/* UNUSED                     0x40000000     (E) */
/* UNUSED                     0x80000000     (F) */

/* Grouping masks */
#define MOVE_MODES (SPRINTING | EVADING | DODGING)
#define MOVE_MODES_LOCK (SPRINTING | EVADING)

/* Flags for mode handling */
constexpr int MODE_EVADE = 0x1;
constexpr int MODE_SPRINT = 0x2;
constexpr int MODE_ON = 0x4;
constexpr int MODE_OFF = 0x8;
constexpr int MODE_DODGE = 0x10;
constexpr int MODE_DG_USED = 0x20;

/* MechFullRecycle check flags */
constexpr int CHECK_WEAPS = 0x1;
constexpr int CHECK_PHYS = 0x2;
#define CHECK_BOTH (CHECK_WEAPS | CHECK_PHYS)

/* Macros for accessing some parts */

/* critstatus element */
constexpr int GYRO_DESTROYED = 0x00000001;         /* (a) */
constexpr int SENSORS_DAMAGED = 0x00000002;        /* (b) */
constexpr int TAG_DESTROYED = 0x00000004;          /* (c) */
constexpr int HIDDEN = 0x00000008;                 /* (d) */
constexpr int GYRO_DAMAGED = 0x00000010;           /* (e) */
constexpr int HIP_DAMAGED = 0x00000020;            /* (f) */
constexpr int LIFE_SUPPORT_DESTROYED = 0x00000040; /* (g) */
constexpr int ANGEL_ECM_DESTROYED = 0x00000080;    /* (h) */
constexpr int C3I_DESTROYED = 0x00000100;          /* (i) */
constexpr int NSS_DESTROYED = 0x00000200;          /* (j) */
constexpr int SLITE_DEST = 0x00000400;             /* (k) */
constexpr int SLITE_LIT = 0x00000800;              /* (l) */
constexpr int LOAD_OK = 0x00001000;          /* (m) Carried load recalculated */
constexpr int OWEIGHT_OK = 0x00002000;       /* (n) Own weight recalculated */
constexpr int SPEED_OK = 0x00004000;         /* (o) Total speed recalculated */
constexpr int HEATCUTOFF = 0x00008000;       /* (p) */
constexpr int TOWABLE = 0x00010000;          /* (q) */
constexpr int HIP_DESTROYED = 0x00020000;    /* (r) */
constexpr int TC_DESTROYED = 0x00040000;     /* (s) */
constexpr int C3_DESTROYED = 0x00080000;     /* (t) */
constexpr int ECM_DESTROYED = 0x00100000;    /* (u) */
constexpr int BEAGLE_DESTROYED = 0x00200000; /* (v) */
constexpr int JELLIED = 0x00400000;          /* (w) Got inferno gel on us */
constexpr int PC_INITIALIZED =
    0x00800000;                      /* (x) PC-initialization done already */
constexpr int SPINNING = 0x01000000; /* (y) */
constexpr int CLAIRVOYANT =
    0x02000000; /* (z) See everything, regardless of blocked */
constexpr int INVISIBLE = 0x04000000;    /* (A) Unable to be seen by anyone */
constexpr int CHEAD = 0x08000000;        /* (B) Altered heading */
constexpr int OBSERVATORIC = 0x10000000; /* (C) */
constexpr int BLOODHOUND_DESTROYED = 0x20000000; /* (D) */
#define MECH_STUNNED                                                           \
  0x40000000 /* (E) Is the mech stunned (Exile stun code)                      \
              */

/* tankcritstatus element */
constexpr int TURRET_LOCKED = 0x01;
constexpr int TURRET_JAMMED = 0x02; /* can be fixed by player */
constexpr int DUG_IN = 0x04;
constexpr int DIGGING_IN = 0x08;
#define CREW_STUNNED                                                           \
  0x10 /* can't go over cruise speed, make any attacks at all, use radio. IE,  \
          can't do jack but turn */
constexpr int TAIL_ROTOR_DESTROYED = 0x20;

/* specials element: used to tell quickly what type of tech the mech has */
constexpr int TRIPLE_MYOMER_TECH = 0x01;
constexpr int CL_ANTI_MISSILE_TECH = 0x02;
constexpr int IS_ANTI_MISSILE_TECH = 0x04;
constexpr int DOUBLE_HEAT_TECH = 0x08;
constexpr int MASC_TECH = 0x10;
constexpr int CLAN_TECH = 0x20;
constexpr int FLIPABLE_ARMS = 0x40;
constexpr int C3_MASTER_TECH = 0x80;
constexpr int C3_SLAVE_TECH = 0x100;
constexpr int ARTEMIS_IV_TECH = 0x200;
constexpr int ECM_TECH = 0x400;
constexpr int BEAGLE_PROBE_TECH = 0x800;
constexpr int SALVAGE_TECH = 0x1000; /* 2x 'mech carrying capacity */
constexpr int CARGO_TECH = 0x2000;   /* 2x cargo carrying capacity */
constexpr int SLITE_TECH = 0x4000;
constexpr int LIGHT_BAP_TECH = 0x8000; /* Removed the Loader_Tech... */
constexpr int AA_TECH = 0x10000;
constexpr int NS_TECH = 0x20000;
constexpr int SS_ABILITY = 0x40000; /* Has sixth sense */
constexpr int FF_TECH = 0x80000;    /* Has ferro-fib. armor */
constexpr int ES_TECH = 0x100000;   /* Has endo-steel internals */
constexpr int XL_TECH = 0x200000;
constexpr int ICE_TECH = 0x400000;  /* ICE engine */
constexpr int FORCE_SHS = 0x800000; /* Was Lifter */
constexpr int LE_TECH = 0x1000000;  /* Light engine */
constexpr int XXL_TECH = 0x2000000;
constexpr int CE_TECH = 0x4000000;
constexpr int REINFI_TECH = 0x8000000;
constexpr int COMPI_TECH = 0x10000000;
constexpr int HARDA_TECH = 0x20000000;
constexpr int CRITPROOF_TECH = 0x40000000;
/* 0x80000000 can not be used. */

/*critstatus2 element */
constexpr int HDGYRO_DAMAGED = 0x01;      /* (a) HDGYRO is damaged */
constexpr int LIGHT_BAP_DESTROYED = 0x02; /* (b) LIGHT_BAP Sensor Destroyed */

/* specials2 element: used to tell quickly what type of tech the mech has */
constexpr int STEALTH_ARMOR_TECH = 0x01; /* Stealth armor */
constexpr int HVY_FF_ARMOR_TECH =
    0x02; /* Heavy FF. 1.24 armor multi. 21 slots. */
constexpr int LASER_REF_ARMOR_TECH = 0x04; /* Not yet implemented */
constexpr int REACTIVE_ARMOR_TECH = 0x08;  /* Not yet implemented */
constexpr int NULLSIGSYS_TECH = 0x10;      /* Null signature system */
constexpr int C3I_TECH = 0x20;             /* Improved C3 */
constexpr int SUPERCHARGER_TECH = 0x40;    /* Not yet implemented */
constexpr int IMPROVED_JJ_TECH = 0x80;
constexpr int MECHANICAL_JJ_TECH = 0x100;    /* Not yet implemented */
constexpr int COMPACT_HS_TECH = 0x200;       /* Not yet implemented */
constexpr int LASER_HS_TECH = 0x400;         /* Not yet implemented */
constexpr int BLOODHOUND_PROBE_TECH = 0x800; /* BLoodhound Active Probe */
constexpr int ANGEL_ECM_TECH = 0x1000;       /* Angel ECM suite */
constexpr int WATCHDOG_TECH = 0x2000;        /* Not yet implemented */
constexpr int LT_FF_ARMOR_TECH =
    0x4000;                      /* Heavy FF. 1.06 armor multi. 7 slots. */
constexpr int TAG_TECH = 0x8000; /* Target Aquisition Gear */
constexpr int OMNIMECH_TECH = 0x10000; /* Is an omni mech */
constexpr int ARTEMISV_TECH = 0x20000; /* Not yet implemented */
constexpr int CAMO_TECH = 0x40000;     /* Allows any unit to 'hide' */
constexpr int CARRIER_TECH = 0x80000;  /* Can be used as a carrier of mechs */
#define WATERPROOF_TECH                                                        \
  0x100000 /* Can the unit go underwater without problems                      \
              for use with tanks */
constexpr int XLGYRO_TECH = 0x200000;
constexpr int HDGYRO_TECH = 0x400000;
constexpr int CGYRO_TECH = 0x800000;
constexpr int TCOMP_TECH = 0x1000000;
constexpr int SMALLCOCKPIT_TECH = 0x2000000;

/* Infantry specials */
constexpr int INF_SWARM_TECH =
    0x01; /* Infantry/BSuits can swarm unfriendlies */
constexpr int INF_MOUNT_TECH = 0x02; /* Infantry/BSuits can mount friendlies */
constexpr int INF_ANTILEG_TECH =
    0x04; /* Infantry/BSuits can make anti-leg attacks */
#define CS_PURIFIER_STEALTH_TECH                                               \
  0x08 /* CS Purifier stealth technology. +3 at 0MP, +2 at 1MP, +1 at 2MP, +3  \
          at 3+MP. */
#define DC_KAGE_STEALTH_TECH                                                   \
  0x10 /* DC stealth technology. +3 at med, +6 at long. No BAP. */
#define FWL_ACHILEUS_STEALTH_TECH                                              \
  0x20 /* FWL stealth technology. +1 at short, +4 at medium, +7 at long. No    \
          BAP. */
#define FC_INFILTRATOR_STEALTH_TECH                                            \
  0x40 /* FC stealth technology. +3 at med, +6 at long. No BAP. */
#define FC_INFILTRATORII_STEALTH_TECH                                          \
  0x80 /* FC stealth II technology. +1 at short, +3 at med, +6 at long. No     \
          BAP. ECM in same hex. */
#define MUST_JETTISON_TECH                                                     \
  0x100 /* Special considerations for some suits. Must jettison backpack       \
           before they can jump/swarm/anti-leg */
#define CAN_JETTISON_TECH                                                      \
  0x200 /* Whether or not the unit can jettison its backpack */

#define STEALTH_TECH                                                           \
  (CS_PURIFIER_STEALTH_TECH | DC_KAGE_STEALTH_TECH |                           \
   FWL_ACHILEUS_STEALTH_TECH | FC_INFILTRATOR_STEALTH_TECH |                   \
   FC_INFILTRATORII_STEALTH_TECH)

/* TargComp types */
constexpr int TARGCOMP_NORMAL = 0;
constexpr int TARGCOMP_SHORT = 1;
constexpr int TARGCOMP_LONG = 2;
constexpr int TARGCOMP_MULTI = 3;
constexpr int TARGCOMP_AA = 4;

/*
        Notes on unimplemented stuff:

        - Laser Reflective Armor:
                - made for better protection against energy weapons
                - 10 crits for IS, 5 for Clan
                - 1 vehicle items for IS, 1 for Clan
                - 16 pts/ton
                - Absorbs 2 pts of damager per pt of armor from energy weapon
   attacks.
                        - Single pt attacks still use up 1 pt of armor.
                - Due to brittleness, double damage from physical and falling
                - Does not apply to unarmored sections

        - Reactive Armor:
                - made for better protection against missile attacks
                - 14 crits for IS, 7 for Clan
                - 2 vehicle items for IS, 1 for Clan
                - 16 pts/ton
                - Can not be mounted on OmniMechs
                - Absorbs 2 pts of damager per pt of armor from missile weapon
   attacks.
                        - Single pt attacks still use up 1 pt of armor.
                - Critical hits on reactive armor slots should be re-rolled, but
   also roll 2d6. On roll of 2 chain reaction occurs and destroys all remaining
                        armor on section (front and back) and cases 1 point of
   internal damage with normal chance of critical hit.
                - Does not apply to unarmored sections

        - Improved JJs
                - Weigh twice as much as std JJs
                -	Take up twice as many crits slots as std JJs
                - Can jump as high as running movement
                - Heat produced by jumping is 1 pt per 2 hexes
                        - Minimum of 3 heat

        - Mechanical Jump Boosters (Mattress Springs Of Doom)
                - Water does not affect them -- can jump out of water
                - System takes up all crits slots in all legs
                - Any critical hit disabled whole system
                - Weighs 5 percent of mech's tonnage * jumping range
                - Range not limited by walking/running movement
                - Mech can not turn while jumping
                - Can not DFA
                - Can mount std JJs and mechanical, but can not use both at the
   same time

        - Compact HS
                - Weighs 1.5 tons
                - Can fit 2 in each crit slot
                - Still gets 10 'free heatsinks'
                - Number of HS 'in engine' is doubled
                - Critical hit destroys both HS
                - IS only
                - Can not be used by vehicles

        - Laser HS
                - Not affected by water
                - Act as DHS for heat dissipation
                - Add 1 to ammo explosion roll
                - At night/dusk
                        - If mech takes any action to generate heat, subtract 1
   from night/dusk modifier
                        - If mech overheats, remove night/dusk modifier
                - Can not be used by vehicles
                - Clan only

        - Watchdog System
                - Clan only
                - Acts as ECM suite and active probe
                - 1 ton, 1 crit

        - Artemis V
                - Clan only
                - Just like ArtemisIV
                - Weighs 1.5 tons and takes up 2 crit slots
                - Not compatible with ArtemisIV
                - -1 BTH
                - Add 3 to missile hit roll
                - Blocked by ECM

        - iNarc Nemesis pods
                - If friendly mech fires SemiGuided or NARC missiles...
                - If Nemesis'd unit between firer and target, resolve attack
                  as if it was fired at the Nemesis'd unit. Add +1 BTH. If
   attack misses missiles continue on to normal target.
                - If multiple Nemesis'd units are between firer and target,
   resolve attack on each in succession from closest unit until all shots hit or
   attack continues on to real target
 */

/* Status stuff for common_checks function */
constexpr int MECH_STARTED = 0x1;
constexpr int MECH_PILOT = 0x2;
constexpr int MECH_PILOT_CON = 0x4;
constexpr int MECH_MAP = 0x8;
constexpr int MECH_CONSISTENT = 0x10;
constexpr int MECH_PILOTONLY = 0x20;
#define MECH_USUAL                                                             \
  (MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOT | MECH_STARTED)
#define MECH_USUALS (MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOT)
#define MECH_USUALSP (MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON)
#define MECH_USUALSM (MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT)
#define MECH_USUALM                                                            \
  (MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT | MECH_STARTED)
#define MECH_USUALO                                                            \
  (MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOT | MECH_STARTED |   \
   MECH_PILOTONLY)
#define MECH_USUALSO                                                           \
  (MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOT | MECH_PILOTONLY)
#define MECH_USUALSPO                                                          \
  (MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOTONLY)
#define MECH_USUALSMO                                                          \
  (MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT | MECH_PILOTONLY)
#define MECH_USUALMO                                                           \
  (MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT | MECH_STARTED |              \
   MECH_PILOTONLY)
