/* Defines BattleTech unit status types. */

#pragma once

#include "section_types.h"
/* The persisted status words keep their four-byte integer representation. */

typedef enum MechStatus : int {
  MECH_STATUS_NONE = 0,
  MECH_STATUS_LANDED = 0x00000001,        /* (a) For VTOL use only */
  MECH_STATUS_TORSO_RIGHT = 0x00000002,   /* (b) Torso heading -= 60 degrees */
  MECH_STATUS_TORSO_LEFT = 0x00000004,    /* (c) Torso heading += 60 degrees */
  MECH_STATUS_STARTED = 0x00000008,       /* (d) Mech is warmed up */
  MECH_STATUS_PARTIAL_COVER = 0x00000010, /* (e) */
  MECH_STATUS_DESTROYED = 0x00000020,     /* (f) */
  MECH_STATUS_JUMPING = 0x00000040,       /* (g) Handled in UPDATE */
  MECH_STATUS_FALLEN = 0x00000080,        /* (h) */
  MECH_STATUS_DFA_ATTACK = 0x00000100,    /* (i) */
  MECH_STATUS_PERFORMING_ACTION =
      0x00000200, /* (j) Unit is performing an action controlled by softcode */
  MECH_STATUS_FLIPPED_ARMS = 0x00000400, /* (k) */
  MECH_STATUS_AMS_ENABLED =
      0x00000800, /* (l) Only settable with anti-missile tech */
  MECH_STATUS_EXPLODE_SAFE =
      0x00001000, /* (m) Prevents a unit from exploding ammunition */
  MECH_STATUS_UNCONSCIOUS = 0x00002000,   /* (n) Pilot is unconscious */
  MECH_STATUS_TOWED = 0x00004000,         /* (o) Someone's towing us */
  MECH_STATUS_LOCK_TARGET = 0x00008000,   /* (p) We mean business */
  MECH_STATUS_LOCK_BUILDING = 0x00010000, /* (q) Hit building */
  MECH_STATUS_LOCK_HEX =
      0x00020000, /* (r) Hit hex (clear or ignite, depending on weapon) */
  MECH_STATUS_LOCK_HEX_IGN = 0x00040000, /* (s) */
  MECH_STATUS_LOCK_HEX_CLR = 0x00080000, /* (t) */
  MECH_STATUS_MASC_ENABLED = 0x00100000, /* (u) Using MASC */
  MECH_STATUS_BLINDED = 0x00200000, /* (v) Pilot has been blinded momentarily */
  MECH_STATUS_COMBAT_SAFE = 0x00400000, /* (w) Can't be hurt */
  MECH_STATUS_AUTOCON_WHEN_SHUTDOWN =
      0x00800000,                 /* (x) Autocon sees it even when shutdown */
  MECH_STATUS_FIRED = 0x01000000, /* (y) Fired at something */
  MECH_STATUS_SCHARGE_ENABLED = 0x02000000,  /* (z) */
  MECH_STATUS_HULLDOWN = 0x04000000,         /* (A) */
  MECH_STATUS_UNDERSPECIAL = 0x08000000,     /* (B) */
  MECH_STATUS_UNDERGRAVITY = 0x10000000,     /* (C) */
  MECH_STATUS_UNDERTEMPERATURE = 0x20000000, /* (D) */
  MECH_STATUS_UNDERVACUUM = 0x40000000,      /* (E) */
  /* UNUSED                     0x80000000     (F) */
  MECH_STATUS_CONDITIONS = MECH_STATUS_UNDERSPECIAL | MECH_STATUS_UNDERGRAVITY |
                           MECH_STATUS_UNDERTEMPERATURE |
                           MECH_STATUS_UNDERVACUUM,
  MECH_STATUS_LOCK_MODES = MECH_STATUS_LOCK_TARGET | MECH_STATUS_LOCK_BUILDING |
                           MECH_STATUS_LOCK_HEX | MECH_STATUS_LOCK_HEX_IGN |
                           MECH_STATUS_LOCK_HEX_CLR,
} MechStatus;

typedef enum MechStatus2 : int {
  MECH_STATUS2_NONE = 0,
  MECH_STATUS2_ECM_ENABLED = 0x00000001,     /* (a) Unit ECM is enabled */
  MECH_STATUS2_ECCM_ENABLED = 0x00000002,    /* (b) Unit ECCM is enabled */
  MECH_STATUS2_ECM_DISTURBANCE = 0x00000004, /* (c) Unit affected by ECM */
  MECH_STATUS2_ECM_PROTECTED = 0x00000008,   /* (d) Unit protected by ECM */
  MECH_STATUS2_ECM_COUNTERED =
      0x00000010, /* (e) ECM countered by enemy ECCM within range */
  MECH_STATUS2_SLITE_ON = 0x00000020, /* (f) Unit SLITE is enabled */
  MECH_STATUS2_STH_ARMOR_ON =
      0x00000040, /* (g) Unit has steath armor enabled */
  MECH_STATUS2_NULLSIGSYS_ON = 0x00000080, /* (h) Unit has nullsig enabled */
  MECH_STATUS2_ANGEL_ECM_ENABLED =
      0x00000100, /* (i) Unit Angel ECM is enabled */
  MECH_STATUS2_ANGEL_ECCM_ENABLED =
      0x00000200, /* (j) Unit Angel ECCM is enabled */
  MECH_STATUS2_ANGEL_ECM_PROTECTED =
      0x00000400, /* (k) Unit protected by Angel ECM */
  MECH_STATUS2_ANGEL_ECM_DISTURBED =
      0x00000800, /* (l) Unit affected by Angel ECM */
  MECH_STATUS2_PER_ECM_ENABLED =
      0x00001000, /* (m) Unit Personal ECM is enabled */
  MECH_STATUS2_PER_ECCM_ENABLED =
      0x00002000, /* (n) Unit Personal ECCM is enabled */
  MECH_STATUS2_AUTOTURN_TURRET =
      0x00004000, /* (o) Auto-turret enabled against locked target */
  /* UNUSED                     0x00008000     (p) */
  MECH_STATUS2_SPRINTING = 0x00010000, /* (q) Unit is Sprinting */
  MECH_STATUS2_EVADING = 0x00020000,   /* (r) Unit is Evading */
  MECH_STATUS2_DODGING = 0x00040000,   /* (s) Unit is Dodging */
  MECH_STATUS2_ATTACKEMIT_MECH =
      0x00080000, /* (t) Unit attacks sent to MechAttackEmits channel */
  MECH_STATUS2_UNIT_MOUNTED =
      0x00100000, /* (u) Unit has been mounted by a suit */
  MECH_STATUS2_UNIT_MOUNTING =
      0x00200000,                      /* (v) Unit is mounting another unit */
  MECH_STATUS2_FORTIFIED = 0x00400000, /* (w) */
  MECH_STATUS2_WEAPONS_HOLD = 0x00800000, /* (x) Unit is unable to fire */
  MECH_STATUS2_NO_GUN_XP =
      0x01000000, /* (y) Don't give gun xp if we fire at this */
  /* This bit is used by the existing supercharger movement state. */
  MECH_STATUS2_SCHARGE_ENABLED = 0x02000000,
  /* UNUSED                     0x04000000     (A) */
  /* UNUSED                     0x08000000     (B) */
  /* UNUSED                     0x10000000     (C) */
  /* UNUSED                     0x20000000     (D) */
  /* UNUSED                     0x40000000     (E) */
  /* UNUSED                     0x80000000     (F) */
  MECH_STATUS2_MOVE_MODES =
      MECH_STATUS2_SPRINTING | MECH_STATUS2_EVADING | MECH_STATUS2_DODGING,
  MECH_STATUS2_MOVE_MODES_LOCK = MECH_STATUS2_SPRINTING | MECH_STATUS2_EVADING,
} MechStatus2;

typedef enum MechSpecialsStatus : int {
  MECH_SPECIALS_STATUS_NONE = 0,
} MechSpecialsStatus;

static inline bool mech_status_has(MechStatus status, MechStatus mask) {
  return (((int)status & (int)mask) != 0);
}

static inline MechStatus mech_status_mask(MechStatus status, MechStatus mask) {
  return (MechStatus)((int)status & (int)mask);
}

static inline void mech_status_set(MechStatus *status, MechStatus mask) {
  *status = (MechStatus)((int)*status | (int)mask);
}

static inline void mech_status_clear(MechStatus *status, MechStatus mask) {
  *status = (MechStatus)((int)*status & ~(int)mask);
}

static inline bool mech_status2_has(MechStatus2 status, MechStatus2 mask) {
  return (((int)status & (int)mask) != 0);
}

static inline MechStatus2 mech_status2_mask(MechStatus2 status,
                                            MechStatus2 mask) {
  return (MechStatus2)((int)status & (int)mask);
}

static inline void mech_status2_set(MechStatus2 *status, MechStatus2 mask) {
  *status = (MechStatus2)((int)*status | (int)mask);
}

static inline void mech_status2_clear(MechStatus2 *status, MechStatus2 mask) {
  *status = (MechStatus2)((int)*status & ~(int)mask);
}

static inline bool mech_specials_status_has(MechSpecialsStatus status,
                                            MechSpecialsStatus mask) {
  return (((int)status & (int)mask) != 0);
}

static inline MechSpecialsStatus
mech_specials_status_mask(MechSpecialsStatus status, MechSpecialsStatus mask) {
  return (MechSpecialsStatus)((int)status & (int)mask);
}

static inline void mech_specials_status_set(MechSpecialsStatus *status,
                                            MechSpecialsStatus mask) {
  *status = (MechSpecialsStatus)((int)*status | (int)mask);
}

static inline void mech_specials_status_clear(MechSpecialsStatus *status,
                                              MechSpecialsStatus mask) {
  *status = (MechSpecialsStatus)((int)*status & ~(int)mask);
}

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
constexpr int CHECK_BOTH = CHECK_WEAPS | CHECK_PHYS;

/* Macros for accessing some parts */

typedef enum MechCritStatus : int {
  MECH_CRIT_STATUS_NONE = 0,
  MECH_CRIT_STATUS_GYRO_DESTROYED = 0x00000001,         /* (a) */
  MECH_CRIT_STATUS_SENSORS_DAMAGED = 0x00000002,        /* (b) */
  MECH_CRIT_STATUS_TAG_DESTROYED = 0x00000004,          /* (c) */
  MECH_CRIT_STATUS_HIDDEN = 0x00000008,                 /* (d) */
  MECH_CRIT_STATUS_GYRO_DAMAGED = 0x00000010,           /* (e) */
  MECH_CRIT_STATUS_HIP_DAMAGED = 0x00000020,            /* (f) */
  MECH_CRIT_STATUS_LIFE_SUPPORT_DESTROYED = 0x00000040, /* (g) */
  MECH_CRIT_STATUS_ANGEL_ECM_DESTROYED = 0x00000080,    /* (h) */
  MECH_CRIT_STATUS_C3I_DESTROYED = 0x00000100,          /* (i) */
  MECH_CRIT_STATUS_NSS_DESTROYED = 0x00000200,          /* (j) */
  MECH_CRIT_STATUS_SLITE_DEST = 0x00000400,             /* (k) */
  MECH_CRIT_STATUS_SLITE_LIT = 0x00000800,              /* (l) */
  MECH_CRIT_STATUS_LOAD_OK = 0x00001000,    /* (m) Carried load recalculated */
  MECH_CRIT_STATUS_OWEIGHT_OK = 0x00002000, /* (n) Own weight recalculated */
  MECH_CRIT_STATUS_SPEED_OK = 0x00004000,   /* (o) Total speed recalculated */
  MECH_CRIT_STATUS_HEATCUTOFF = 0x00008000, /* (p) */
  MECH_CRIT_STATUS_TOWABLE = 0x00010000,    /* (q) */
  MECH_CRIT_STATUS_HIP_DESTROYED = 0x00020000,    /* (r) */
  MECH_CRIT_STATUS_TC_DESTROYED = 0x00040000,     /* (s) */
  MECH_CRIT_STATUS_C3_DESTROYED = 0x00080000,     /* (t) */
  MECH_CRIT_STATUS_ECM_DESTROYED = 0x00100000,    /* (u) */
  MECH_CRIT_STATUS_BEAGLE_DESTROYED = 0x00200000, /* (v) */
  MECH_CRIT_STATUS_JELLIED = 0x00400000, /* (w) Got inferno gel on us */
  MECH_CRIT_STATUS_PC_INITIALIZED =
      0x00800000, /* (x) PC-initialization done already */
  MECH_CRIT_STATUS_SPINNING = 0x01000000, /* (y) */
  MECH_CRIT_STATUS_CLAIRVOYANT =
      0x02000000, /* (z) See everything, regardless of blocked */
  MECH_CRIT_STATUS_INVISIBLE = 0x04000000, /* (A) Unable to be seen by anyone */
  MECH_CRIT_STATUS_CHEAD = 0x08000000,     /* (B) Altered heading */
  MECH_CRIT_STATUS_OBSERVATORIC = 0x10000000,         /* (C) */
  MECH_CRIT_STATUS_BLOODHOUND_DESTROYED = 0x20000000, /* (D) */
  MECH_CRIT_STATUS_MECH_STUNNED = 0x40000000,         /* (E) Mech is stunned */
} MechCritStatus;

typedef enum MechTankCritStatus : int {
  MECH_TANK_CRIT_STATUS_NONE = 0,
  MECH_TANK_CRIT_STATUS_TURRET_LOCKED = 0x01,
  MECH_TANK_CRIT_STATUS_TURRET_JAMMED = 0x02, /* can be fixed by player */
  MECH_TANK_CRIT_STATUS_DUG_IN = 0x04,
  MECH_TANK_CRIT_STATUS_DIGGING_IN = 0x08,
  MECH_TANK_CRIT_STATUS_CREW_STUNNED =
      0x10, /* Crew can only turn: no flank speed, attacks, or radio */
  MECH_TANK_CRIT_STATUS_TAIL_ROTOR_DESTROYED = 0x20,
} MechTankCritStatus;

typedef enum MechCritStatus2 : int {
  MECH_CRIT_STATUS2_NONE = 0,
  MECH_CRIT_STATUS2_HDGYRO_DAMAGED = 0x01, /* (a) HDGYRO is damaged */
  MECH_CRIT_STATUS2_LIGHT_BAP_DESTROYED =
      0x02, /* (b) LIGHT_BAP Sensor Destroyed */
} MechCritStatus2;

static inline bool mech_crit_status_has(MechCritStatus status,
                                        MechCritStatus mask) {
  return (((int)status & (int)mask) != 0);
}

static inline MechCritStatus mech_crit_status_mask(MechCritStatus status,
                                                   MechCritStatus mask) {
  return (MechCritStatus)((int)status & (int)mask);
}

static inline void mech_crit_status_set(MechCritStatus *status,
                                        MechCritStatus mask) {
  *status = (MechCritStatus)((int)*status | (int)mask);
}

static inline void mech_crit_status_clear(MechCritStatus *status,
                                          MechCritStatus mask) {
  *status = (MechCritStatus)((int)*status & ~(int)mask);
}

static inline bool mech_tank_crit_status_has(MechTankCritStatus status,
                                             MechTankCritStatus mask) {
  return (((int)status & (int)mask) != 0);
}

static inline MechTankCritStatus
mech_tank_crit_status_mask(MechTankCritStatus status, MechTankCritStatus mask) {
  return (MechTankCritStatus)((int)status & (int)mask);
}

static inline void mech_tank_crit_status_set(MechTankCritStatus *status,
                                             MechTankCritStatus mask) {
  *status = (MechTankCritStatus)((int)*status | (int)mask);
}

static inline void mech_tank_crit_status_clear(MechTankCritStatus *status,
                                               MechTankCritStatus mask) {
  *status = (MechTankCritStatus)((int)*status & ~(int)mask);
}

static inline bool mech_crit_status2_has(MechCritStatus2 status,
                                         MechCritStatus2 mask) {
  return (((int)status & (int)mask) != 0);
}

static inline MechCritStatus2 mech_crit_status2_mask(MechCritStatus2 status,
                                                     MechCritStatus2 mask) {
  return (MechCritStatus2)((int)status & (int)mask);
}

static inline void mech_crit_status2_set(MechCritStatus2 *status,
                                         MechCritStatus2 mask) {
  *status = (MechCritStatus2)((int)*status | (int)mask);
}

static inline void mech_crit_status2_clear(MechCritStatus2 *status,
                                           MechCritStatus2 mask) {
  *status = (MechCritStatus2)((int)*status & ~(int)mask);
}

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
constexpr int WATERPROOF_TECH =
    0x100000; /* Unit can operate underwater without problems */
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
constexpr int CS_PURIFIER_STEALTH_TECH = 0x08;
constexpr int DC_KAGE_STEALTH_TECH = 0x10;
constexpr int FWL_ACHILEUS_STEALTH_TECH = 0x20;
constexpr int FC_INFILTRATOR_STEALTH_TECH = 0x40;
constexpr int FC_INFILTRATORII_STEALTH_TECH = 0x80;
constexpr int MUST_JETTISON_TECH = 0x100;
constexpr int CAN_JETTISON_TECH = 0x200;

constexpr int STEALTH_TECH = CS_PURIFIER_STEALTH_TECH | DC_KAGE_STEALTH_TECH |
                             FWL_ACHILEUS_STEALTH_TECH |
                             FC_INFILTRATOR_STEALTH_TECH |
                             FC_INFILTRATORII_STEALTH_TECH;

/* TargComp types */
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
constexpr int MECH_USUAL =
    MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOT | MECH_STARTED;
constexpr int MECH_USUALS =
    MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOT;
constexpr int MECH_USUALSP = MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON;
constexpr int MECH_USUALSM = MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT;
constexpr int MECH_USUALM =
    MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT | MECH_STARTED;
constexpr int MECH_USUALO = MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON |
                            MECH_PILOT | MECH_STARTED | MECH_PILOTONLY;
constexpr int MECH_USUALSO =
    MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOT | MECH_PILOTONLY;
constexpr int MECH_USUALSPO =
    MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON | MECH_PILOTONLY;
constexpr int MECH_USUALSMO =
    MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT | MECH_PILOTONLY;
constexpr int MECH_USUALMO = MECH_CONSISTENT | MECH_PILOT_CON | MECH_PILOT |
                             MECH_STARTED | MECH_PILOTONLY;
