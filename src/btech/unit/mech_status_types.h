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

#define LANDED 0x00000001        /* (a) For VTOL use only */
#define TORSO_RIGHT 0x00000002   /* (b) Torso heading -= 60 degrees */
#define TORSO_LEFT 0x00000004    /* (c) Torso heading += 60 degrees */
#define STARTED 0x00000008       /* (d) Mech is warmed up */
#define PARTIAL_COVER 0x00000010 /* (e) */
#define DESTROYED 0x00000020     /* (f) */
#define JUMPING 0x00000040       /* (g) Handled in UPDATE */
#define FALLEN 0x00000080        /* (h) */
#define DFA_ATTACK 0x00000100    /* (i) */
#define PERFORMING_ACTION                                                      \
  0x00000200 /* (j) Set if the unit is performing some sort of action.         \
                Controlled by SCode */
#define FLIPPED_ARMS 0x00000400 /* (k) */
#define AMS_ENABLED                                                            \
  0x00000800 /* (l) only settable if mech has ANTI-MISSILE_TECH */
#define EXPLODE_SAFE                                                           \
  0x00001000 /* (m) Used to prevent a unit from doing EXPLODE AMMO */
#define UNCONSCIOUS 0x00002000   /* (n) Pilot is unconscious */
#define TOWED 0x00004000         /* (o) Someone's towing us */
#define LOCK_TARGET 0x00008000   /* (p) We mean business */
#define LOCK_BUILDING 0x00010000 /* (q) Hit building */
#define LOCK_HEX                                                               \
  0x00020000 /* (r) Hit hex (clear / ignite, d'pend on weapon)                 \
              */
#define LOCK_HEX_IGN 0x00040000 /* (s) */
#define LOCK_HEX_CLR 0x00080000 /* (t) */
#define MASC_ENABLED 0x00100000 /* (u) Using MASC */
#define BLINDED                                                                \
  0x00200000 /* (v) Pilot has been blinded momentarily by something */
#define COMBAT_SAFE 0x00400000 /* (w) Can't be hurt */
#define AUTOCON_WHEN_SHUTDOWN                                                  \
  0x00800000                        /* (x) Autocon sees it even when shutdown */
#define FIRED 0x01000000            /* (y) Fired at something */
#define SCHARGE_ENABLED 0x02000000  /* (z) */
#define HULLDOWN 0x04000000         /* (A) */
#define UNDERSPECIAL 0x08000000     /* (B) */
#define UNDERGRAVITY 0x10000000     /* (C) */
#define UNDERTEMPERATURE 0x20000000 /* (D) */
#define UNDERVACUUM 0x40000000      /* (E) */
/* UNUSED                     0x80000000     (F) */

#define CONDITIONS                                                             \
  (UNDERSPECIAL | UNDERGRAVITY | UNDERTEMPERATURE | UNDERVACUUM)
#define LOCK_MODES                                                             \
  (LOCK_TARGET | LOCK_BUILDING | LOCK_HEX | LOCK_HEX_IGN | LOCK_HEX_CLR)

/* status2 element */

/* Specials status element */
#define ECM_ENABLED 0x00000001     /* (a) Unit ECM is enabled */
#define ECCM_ENABLED 0x00000002    /* (b) Unit ECCM is enabled */
#define ECM_DISTURBANCE 0x00000004 /* (c) Unit affected by ECM */
#define ECM_PROTECTED 0x00000008   /* (d) Unit protected by ECM */
#define ECM_COUNTERED                                                          \
  0x00000010                           /* (e) ECM countered by ECCM.           \
                                          This only happens if an enemy ECCM   \
                                          is within range. */
#define SLITE_ON 0x00000020            /* (f) Unit SLITE is enabled */
#define STH_ARMOR_ON 0x00000040        /* (g) Unit has steath armor enabled */
#define NULLSIGSYS_ON 0x00000080       /* (h) Unit has nullsig enabled */
#define ANGEL_ECM_ENABLED 0x00000100   /* (i) Unit Angel ECM is enabled */
#define ANGEL_ECCM_ENABLED 0x00000200  /* (j) Unit Angel ECCM is enabled */
#define ANGEL_ECM_PROTECTED 0x00000400 /* (k) Unit protected by Angel ECM */
#define ANGEL_ECM_DISTURBED 0x00000800 /* (l) Unit affected by Angel ECM */
#define PER_ECM_ENABLED 0x00001000     /* (m) Unit Personal ECM is enabled */
#define PER_ECCM_ENABLED 0x00002000    /* (n) Unit Personal ECCM is enabled */
#define AUTOTURN_TURRET                                                        \
  0x00004000 /* (o) Unit Auto-Turret enabled to locked target */
/* UNUSED                     0x00008000     (p) */
#define SPRINTING 0x00010000 /* (q) Unit is Sprinting */
#define EVADING 0x00020000   /* (r) Unit is Evading */
#define DODGING 0x00040000   /* (s) Unit is Dodging */
#define ATTACKEMIT_MECH                                                        \
  0x00080000 /* (t) Units attacks sent to MechAttackEmits channel */
#define UNIT_MOUNTED 0x00100000  /* (u) Unit has been mounted by a suit */
#define UNIT_MOUNTING 0x00200000 /* (v) Unit is mounting another unit */
#define FORTIFIED 0x00400000     /* (w) */
#define WEAPONS_HOLD 0x00800000  /* (x) Unit is unable to fire */
#define NO_GUN_XP 0x01000000     /* (y) Don't give gun xp if we fire at this */
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
#define MODE_EVADE 0x1
#define MODE_SPRINT 0x2
#define MODE_ON 0x4
#define MODE_OFF 0x8
#define MODE_DODGE 0x10
#define MODE_DG_USED 0x20

/* MechFullRecycle check flags */
#define CHECK_WEAPS 0x1
#define CHECK_PHYS 0x2
#define CHECK_BOTH (CHECK_WEAPS | CHECK_PHYS)

#define MechLockFire(mech)                                                     \
  ((MechStatus(mech) & LOCK_TARGET) &&                                         \
   !(MechStatus(mech) &                                                        \
     (LOCK_BUILDING | LOCK_HEX | LOCK_HEX_IGN | LOCK_HEX_CLR)))

/* Macros for accessing some parts */
#define Blinded(a) (MechStatus(a) & BLINDED)
#define Started(a) (MechStatus(a) & STARTED)
#define Uncon(a) (MechStatus(a) & UNCONSCIOUS)

/* critstatus element */
#define GYRO_DESTROYED 0x00000001         /* (a) */
#define SENSORS_DAMAGED 0x00000002        /* (b) */
#define TAG_DESTROYED 0x00000004          /* (c) */
#define HIDDEN 0x00000008                 /* (d) */
#define GYRO_DAMAGED 0x00000010           /* (e) */
#define HIP_DAMAGED 0x00000020            /* (f) */
#define LIFE_SUPPORT_DESTROYED 0x00000040 /* (g) */
#define ANGEL_ECM_DESTROYED 0x00000080    /* (h) */
#define C3I_DESTROYED 0x00000100          /* (i) */
#define NSS_DESTROYED 0x00000200          /* (j) */
#define SLITE_DEST 0x00000400             /* (k) */
#define SLITE_LIT 0x00000800              /* (l) */
#define LOAD_OK 0x00001000                /* (m) Carried load recalculated */
#define OWEIGHT_OK 0x00002000             /* (n) Own weight recalculated */
#define SPEED_OK 0x00004000               /* (o) Total speed recalculated */
#define HEATCUTOFF 0x00008000             /* (p) */
#define TOWABLE 0x00010000                /* (q) */
#define HIP_DESTROYED 0x00020000          /* (r) */
#define TC_DESTROYED 0x00040000           /* (s) */
#define C3_DESTROYED 0x00080000           /* (t) */
#define ECM_DESTROYED 0x00100000          /* (u) */
#define BEAGLE_DESTROYED 0x00200000       /* (v) */
#define JELLIED 0x00400000                /* (w) Got inferno gel on us */
#define PC_INITIALIZED 0x00800000 /* (x) PC-initialization done already */
#define SPINNING 0x01000000       /* (y) */
#define CLAIRVOYANT 0x02000000  /* (z) See everything, regardless of blocked */
#define INVISIBLE 0x04000000    /* (A) Unable to be seen by anyone */
#define CHEAD 0x08000000        /* (B) Altered heading */
#define OBSERVATORIC 0x10000000 /* (C) */
#define BLOODHOUND_DESTROYED 0x20000000 /* (D) */
#define MECH_STUNNED                                                           \
  0x40000000 /* (E) Is the mech stunned (Exile stun code)                      \
              */

/* tankcritstatus element */
#define TURRET_LOCKED 0x01
#define TURRET_JAMMED 0x02 /* can be fixed by player */
#define DUG_IN 0x04
#define DIGGING_IN 0x08
#define CREW_STUNNED                                                           \
  0x10 /* can't go over cruise speed, make any attacks at all, use radio. IE,  \
          can't do jack but turn */
#define TAIL_ROTOR_DESTROYED 0x20

/* specials element: used to tell quickly what type of tech the mech has */
#define TRIPLE_MYOMER_TECH 0x01
#define CL_ANTI_MISSILE_TECH 0x02
#define IS_ANTI_MISSILE_TECH 0x04
#define DOUBLE_HEAT_TECH 0x08
#define MASC_TECH 0x10
#define CLAN_TECH 0x20
#define FLIPABLE_ARMS 0x40
#define C3_MASTER_TECH 0x80
#define C3_SLAVE_TECH 0x100
#define ARTEMIS_IV_TECH 0x200
#define ECM_TECH 0x400
#define BEAGLE_PROBE_TECH 0x800
#define SALVAGE_TECH 0x1000 /* 2x 'mech carrying capacity */
#define CARGO_TECH 0x2000   /* 2x cargo carrying capacity */
#define SLITE_TECH 0x4000
#define LIGHT_BAP_TECH 0x8000 /* Removed the Loader_Tech... */
#define AA_TECH 0x10000
#define NS_TECH 0x20000
#define SS_ABILITY 0x40000 /* Has sixth sense */
#define FF_TECH 0x80000    /* Has ferro-fib. armor */
#define ES_TECH 0x100000   /* Has endo-steel internals */
#define XL_TECH 0x200000
#define ICE_TECH 0x400000  /* ICE engine */
#define FORCE_SHS 0x800000 /* Was Lifter */
#define LE_TECH 0x1000000  /* Light engine */
#define XXL_TECH 0x2000000
#define CE_TECH 0x4000000
#define REINFI_TECH 0x8000000
#define COMPI_TECH 0x10000000
#define HARDA_TECH 0x20000000
#define CRITPROOF_TECH 0x40000000
/* 0x80000000 can not be used. */

/*critstatus2 element */
#define HDGYRO_DAMAGED 0x01      /* (a) HDGYRO is damaged */
#define LIGHT_BAP_DESTROYED 0x02 /* (b) LIGHT_BAP Sensor Destroyed */

/* specials2 element: used to tell quickly what type of tech the mech has */
#define STEALTH_ARMOR_TECH 0x01   /* Stealth armor */
#define HVY_FF_ARMOR_TECH 0x02    /* Heavy FF. 1.24 armor multi. 21 slots. */
#define LASER_REF_ARMOR_TECH 0x04 /* Not yet implemented */
#define REACTIVE_ARMOR_TECH 0x08  /* Not yet implemented */
#define NULLSIGSYS_TECH 0x10      /* Null signature system */
#define C3I_TECH 0x20             /* Improved C3 */
#define SUPERCHARGER_TECH 0x40    /* Not yet implemented */
#define IMPROVED_JJ_TECH 0x80
#define MECHANICAL_JJ_TECH 0x100    /* Not yet implemented */
#define COMPACT_HS_TECH 0x200       /* Not yet implemented */
#define LASER_HS_TECH 0x400         /* Not yet implemented */
#define BLOODHOUND_PROBE_TECH 0x800 /* BLoodhound Active Probe */
#define ANGEL_ECM_TECH 0x1000       /* Angel ECM suite */
#define WATCHDOG_TECH 0x2000        /* Not yet implemented */
#define LT_FF_ARMOR_TECH 0x4000     /* Heavy FF. 1.06 armor multi. 7 slots. */
#define TAG_TECH 0x8000             /* Target Aquisition Gear */
#define OMNIMECH_TECH 0x10000       /* Is an omni mech */
#define ARTEMISV_TECH 0x20000       /* Not yet implemented */
#define CAMO_TECH 0x40000           /* Allows any unit to 'hide' */
#define CARRIER_TECH 0x80000        /* Can be used as a carrier of mechs */
#define WATERPROOF_TECH                                                        \
  0x100000 /* Can the unit go underwater without problems                      \
              for use with tanks */
#define XLGYRO_TECH 0x200000
#define HDGYRO_TECH 0x400000
#define CGYRO_TECH 0x800000
#define TCOMP_TECH 0x1000000
#define SMALLCOCKPIT_TECH 0x2000000

/* Infantry specials */
#define INF_SWARM_TECH 0x01   /* Infantry/BSuits can swarm unfriendlies */
#define INF_MOUNT_TECH 0x02   /* Infantry/BSuits can mount friendlies */
#define INF_ANTILEG_TECH 0x04 /* Infantry/BSuits can make anti-leg attacks */
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
#define TARGCOMP_NORMAL 0
#define TARGCOMP_SHORT 1
#define TARGCOMP_LONG 2
#define TARGCOMP_MULTI 3
#define TARGCOMP_AA 4

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
#define MECH_STARTED 0x1
#define MECH_PILOT 0x2
#define MECH_PILOT_CON 0x4
#define MECH_MAP 0x8
#define MECH_CONSISTENT 0x10
#define MECH_PILOTONLY 0x20
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
