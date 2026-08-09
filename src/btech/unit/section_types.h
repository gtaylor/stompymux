/* Defines BattleTech unit section types. */

#pragma once

#include "equipment_types.h"
struct CriticalSlot {
  unsigned char brand;   /* Hold brand number, and damage (upper 4 bits) */
  unsigned char data;    /* Holds information like ammo remaining, etc */
  unsigned short type;   /* Type of item that this is a critical for */
  unsigned int firemode; /* Holds info like rear mount, ultra mode... */
  unsigned int ammomode; /* Holds info for the special ammo type in use */
  unsigned int weapDamageFlags; /* Holds the enhanced critical damage flags */
  short desiredAmmoLoc;         /* Location of the desired ammo bin */
  //    unsigned int recycle;   /* time when it will finish recycling */
};

/* Fire modes */
constexpr int DESTROYED_MODE = 0x00000001; /* the part is destroyed */
constexpr int DISABLED_MODE = 0x00000002;  /* the part is disabled */
constexpr int BROKEN_MODE =
    0x00000004; /* the part is part of a destroyed weapon/item */
constexpr int DAMAGED_MODE =
    0x00000008; /* the part is damaged from an enhanced critical */
constexpr int ON_TC = 0x00000010; /* (T) Set if the wepons mounted with TC */
constexpr int REAR_MOUNT = 0x00000020;   /* (R) set if weapon is rear mounted */
constexpr int HOTLOAD_MODE = 0x00000040; /* (H) Weapon's being hotloaded */
constexpr int HALFTON_MODE = 0x00000080; /* Weapon is in halfton mode */
constexpr int OS_MODE =
    0x00000100; /* (O) In weapon itself : Weapon's one-shot */
constexpr int OS_USED = 0x00000200; /* One-shot ammo _has_ been already used */
constexpr int ULTRA_MODE =
    0x00000400; /* (U) set if weapon is in Ultra firing mode */
constexpr int RFAC_MODE =
    0x00000800; /* (F) the weapon is set as a rapid fire AC */
constexpr int GATTLING_MODE = 0x00001000;     /* (G) For Gattling MGs */
constexpr int RAC_TWOSHOT_MODE = 0x00002000;  /* (2) RAC in two shot mode */
constexpr int RAC_FOURSHOT_MODE = 0x00004000; /* (4) RAC in four shot mode */
constexpr int RAC_SIXSHOT_MODE = 0x00008000;  /* (6) RAC in six shot mode */
constexpr int HEAT_MODE = 0x00010000; /* (H) Toggle a flamer into heat mode */
constexpr int WILL_JETTISON_MODE =
    0x00020000; /* Destroy during a backpack jettison (BSuits) */
constexpr int IS_JETTISONED_MODE =
    0x00040000; /* Slot has been jettisoned (BSuits) */
constexpr int OMNI_BASE_MODE =
    0x00080000; /* Slot is part of an omni mech's base config */
constexpr int ROCKET_FIRED =
    0x00100000; /* Slot's rocket launcher has been fired */

constexpr int RAC_MODES =
    RAC_TWOSHOT_MODE | RAC_FOURSHOT_MODE | RAC_SIXSHOT_MODE;
constexpr int FIRE_MODES = HOTLOAD_MODE | ULTRA_MODE | RFAC_MODE |
                           GATTLING_MODE | RAC_MODES | HEAT_MODE;

/* Ammo modes */
constexpr int LBX_MODE = 0x00000001; /* (L) set if weapon is firing LBX ammo */
constexpr int ARTEMIS_MODE =
    0x00000002; /* (A) artemis compatible missiles/laucher */
constexpr int NARC_MODE =
    0x00000004; /* (N) narc compatible missiles/launcher */
constexpr int CLUSTER_MODE =
    0x00000008; /* (C) Set if weapon is firing cluster ammo */
constexpr int MINE_MODE = 0x00000010; /* (M) Set if weapon's firing mines */
constexpr int SMOKE_MODE =
    0x00000020; /* (S) Set if weapon's firing smoke rounds */
constexpr int INFERNO_MODE =
    0x00000040; /* (I) SRM's loaded with Inferno rounds (cause heat) */
constexpr int SWARM_MODE = 0x00000080; /* (W) LRM's loaded with Swarm rounds */
constexpr int SWARM1_MODE =
    0x00000100; /* (1) LRM's loaded with Swarm1 rounds (FoF) */
constexpr int INARC_EXPLO_MODE =
    0x00000200; /* (X) inarc launcher firing explosive pods */
constexpr int INARC_HAYWIRE_MODE =
    0x00000400; /* (Y) inarc launcher firing haywire pods */
constexpr int INARC_ECM_MODE =
    0x00000800; /* (E) inarc launcher firing ecm pods */
constexpr int INARC_NEMESIS_MODE =
    0x00001000; /* (Z) inarc launcher firing nemesis pods */
constexpr int AC_AP_MODE =
    0x00002000; /* (R) autocannon firing armor piercing rounds */
constexpr int AC_FLECHETTE_MODE =
    0x00004000; /* (F) autocannon firing flechette rounds */
constexpr int AC_INCENDIARY_MODE =
    0x00008000; /* (D) autocannon firing incendiary rounds */
constexpr int AC_PRECISION_MODE =
    0x00010000; /* (P) autocannon firing precision rounds */
constexpr int STINGER_MODE = 0x00020000; /* (T) AntiAir LRM */
constexpr int AC_CASELESS_MODE =
    0x00040000; /* (U) autocannon firing caseless rounds */
constexpr int SGUIDED_MODE =
    0x00080000; /* (G) Semi-Guided LRMs, benefiting when lit by TAG */
constexpr int ATM_ER_MODE = 0x00100000;  /* (R) ATM's in Extended Range mode */
constexpr int ATM_HE_MODE = 0x00200000;  /* (X) ATM's in High Explosive Mode */
constexpr int MML_LRM_MODE = 0x00400000; /* (#) MML in LRM Mode */

constexpr int ARTILLERY_MODES = CLUSTER_MODE | MINE_MODE | SMOKE_MODE;
constexpr int INARC_MODES =
    INARC_EXPLO_MODE | INARC_HAYWIRE_MODE | INARC_ECM_MODE | INARC_NEMESIS_MODE;
constexpr int MISSILE_MODES = ARTEMIS_MODE | NARC_MODE | INFERNO_MODE |
                              SWARM_MODE | SWARM1_MODE | STINGER_MODE |
                              SGUIDED_MODE;
constexpr int AC_MODES = AC_AP_MODE | AC_FLECHETTE_MODE | AC_INCENDIARY_MODE |
                         AC_PRECISION_MODE | AC_CASELESS_MODE;
constexpr int ATM_MODES = ATM_ER_MODE | ATM_HE_MODE;
constexpr int AMMO_MODES = LBX_MODE | AC_MODES | MISSILE_MODES | INARC_MODES |
                           ARTILLERY_MODES | ATM_MODES | MML_LRM_MODE;

/* Enhanced critical damage flags */
constexpr int WEAP_DAM_MODERATE = 0x00000001; /* +1 to hit */
constexpr int WEAP_DAM_EN_FOCUS =
    0x00000002; /* Energy focus misaligned: -1 damage, +1 BTH at range */
constexpr int WEAP_DAM_EN_CRYSTAL =
    0x00000004; /* Energy crystal damaged: +1 heat, possible explosion */
constexpr int WEAP_DAM_BALL_BARREL =
    0x00000008; /* Ballistic barrel damaged: possible weapon jam */
constexpr int WEAP_DAM_BALL_AMMO =
    0x00000010; /* Ballistic ammo feed damaged: fixed ammo, explosion risk */
constexpr int WEAP_DAM_MSL_RANGING =
    0x00000020; /* Missile ranging damaged: +1 BTH at medium and long range */
constexpr int WEAP_DAM_MSL_AMMO =
    0x00000040; /* Missile ammo feed damaged: fixed ammo, explosion risk */

/* Structure for each of the 8 sections */
struct MechSection {
  unsigned char armor;    /* External armor value */
  unsigned char internal; /* Internal armor value */
  unsigned char rear;     /* Rear armor value */
  unsigned char armor_orig;
  unsigned char internal_orig;
  unsigned char rear_orig;
  char basetohit; /* Holds to hit modifiers for weapons in section */
  char config;    /* flags for CASE, etc. */
  char recycle;   /* after physical attack, set counter */
  unsigned short
      specials; /* specials for this section, like attached NARC pods, etc... */
  struct CriticalSlot criticals[NUM_CRITICALS]; /* Criticals */
};

/* Section configurations */
constexpr int CASE_TECH = 0x01;         /* section has CASE technology */
constexpr int SECTION_DESTROYED = 0x02; /* section has been destroyed */
constexpr int SECTION_BREACHED = 0x04;  /* section has been exposed to vacuum */
constexpr int SECTION_FLOODED =
    0x08; /* section has been flooded with water - Kipsta. 8/3/99 */
constexpr int AXED = 0x10; /* arm was used to axe/sword someone */
constexpr int STABILIZERS_DESTROYED =
    0x20; /* vehicle only. Double attacker mod for section weapons */
constexpr int CASEII_TECH = 0x40; /* section has CASE II technology */

/* Section specials */
constexpr int NARC_ATTACHED =
    0x00000001; /* set if mech has a NARC beacon attached. */
constexpr int INARC_HOMING_ATTACHED =
    0x00000002; /* set if mech has an iNARC homing beacon attached. */
constexpr int INARC_HAYWIRE_ATTACHED =
    0x00000004; /* set if mech has an iNARC haywire beacon attached. */
constexpr int INARC_ECM_ATTACHED =
    0x00000008; /* set if mech has an iNARC ecm beacon attached. */
constexpr int INARC_NEMESIS_ATTACHED =
    0x00000010; /* set if mech has an iNARC nemesis beacon attached. */
constexpr int CARRYING_CLUB = 0x00000020; /* carrying a club in this location */

typedef enum UnitClass : int {
  CLASS_MECH = 0,
  CLASS_VEH_GROUND = 1,
  CLASS_VTOL = 2,
  CLASS_VEH_NAVAL = 3,
  CLASS_SPHEROID_DS = 4, /* Spheroid DropShip */
  CLASS_AERO = 5,
  CLASS_MW = 6, /* Ejected MechWarrior */
  CLASS_DS = 7, /* AeroDyne DropShip */
  CLASS_BSUIT = 8,
  CLASS_LAST = 8,
} UnitClass;

typedef enum MechMovementType : int {
  MOVE_BIPED = 0,
  MOVE_TRACK = 1,
  MOVE_WHEEL = 2,
  MOVE_HOVER = 3,
  MOVE_VTOL = 4,
  MOVE_HULL = 5,
  MOVE_FOIL = 6,
  MOVE_FLY = 7,
  MOVE_QUAD = 8,
  MOVE_SUB = 9,
  MOVE_NONE = 10, /* Stationary, for one reason or another */
  MOVENEMENT_LAST = 10,
} MechMovementType;

static_assert(CLASS_VEH_NAVAL == 3 && CLASS_LAST == 8);
static_assert(MOVE_QUAD == 8 && MOVE_SUB == 9 && MOVENEMENT_LAST == 10);

/* Mech Preferences list */
constexpr int MECHPREF_PKILL = 0x00000001;    /* Kill MWs anyway */
constexpr int MECHPREF_SLWARN = 0x00000002;   /* Warn when lit by slite */
constexpr int MECHPREF_AUTOFALL = 0x00000004; /* Jump off cliffs */
constexpr int MECHPREF_NOARMORWARN =
    0x00000008; /* Don't warn when armor is getting low */
constexpr int MECHPREF_NOAMMOWARN =
    0x00000010; /* Don't warn when ammo is getting low */
constexpr int MECHPREF_STANDANYWAY =
    0x00000020; /* Try to stand even when BTH too high */
constexpr int MECHPREF_AUTOCON_SD =
    0x00000040; /* Autocon on non-started units */
constexpr int MECHPREF_NOFRIENDLYFIRE =
    0x00000080; /* Disallow firing on teammates */
constexpr int MECHPREF_TURNMODE =
    0x00000100; /* Tight or Normal for Maneuvering Ace */
constexpr int MECHPREF_BTHDEBUG =
    0x00000200; /* Show BTH Debug or not (Can Get Spammy) */
